use crate::graphics::default_shader;
use crevice::std140::AsStd140;

use crate::{
    context::{Has, HasMut},
    GameError, GameResult,
};

use super::{
    internal_canvas3d::{screen_to_mat, InstanceArrayView3d, InternalCanvas3d},
    BlendMode, Color, DrawParam3d, Drawable3d, GraphicsContext, Image, Mesh3d, Rect, Sampler,
    ScreenImage, Shader, ShaderParams, WgpuContext,
};
use std::{cmp::Ordering, sync::Arc};

/// Alpha mode is how to render a given draw. Opaque has no transparency, discard discards transparent pixels under a certain value. Blend will blend them
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    /// Render this mesh with no transparency
    Opaque,
    /// Render this mesh a transparency cutoff
    Discard {
        /// What transparency to start discarding at. This will be converted to floats so 0 would be 0.0 1 would be 0.1 and so on
        cutoff: i8,
    },
    /// Render this mesh with transparency
    Blend {
        /// The blend mode used when the mesh is transparent
        blend_mode: BlendMode,
    },
}

/// Canvas3d are the main method of drawing 3d meshes in ggez.
///
/// They can draw to any image that is capable of being drawn to (i.e. has been created with [`Image::new_canvas_image()`] or [`ScreenImage`]),
/// or they can draw directly to the screen.
///
// note:
//   Canvas3d does not draw anything itself. It is merely a state-tracking and draw-reordering wrapper around InternalCanvas3d, which does the actual
// drawing.
#[derive(Debug)]
pub struct Canvas3d {
    pub(crate) wgpu: Arc<WgpuContext>,
    pub(crate) defaults: DefaultResources3d,
    draws: Vec<DrawCommand3d>,
    state: DrawState3d,
    original_state: DrawState3d,
    screen: Option<Rect>,
    sort_by: fn(&DrawParam3d, &DrawParam3d) -> Ordering,

    target: Image,
    target_depth: Image,
    resolve: Option<Image>,
    clear: Option<Color>,
}

impl Canvas3d {
    /// Create a new [Canvas3d] from an image. This will allow for drawing to a single color image.
    ///
    /// `clear` will set the image initially to the given color, if a color is provided, or keep it as is, if it's `None`.
    ///
    /// The image must be created for Canvas3d usage, i.e. [`Image::new_canvas_image`()], or [`ScreenImage`], and must only have a sample count of 1.
    #[inline]
    pub fn from_image(
        gfx: &impl Has<GraphicsContext>,
        image: Image,
        clear: impl Into<Option<Color>>,
    ) -> Self {
        Canvas3d::new(gfx, image, None, clear.into())
    }

    /// Helper for [`Canvas3d::from_image`] for construction of a [`Canvas3d`] from a [`ScreenImage`].
    #[inline]
    pub fn from_screen_image(
        gfx: &impl Has<GraphicsContext>,
        image: &mut ScreenImage,
        clear: impl Into<Option<Color>>,
    ) -> Self {
        let gfx = gfx.retrieve();
        let image = image.image(gfx);
        Canvas3d::from_image(gfx, image, clear)
    }

    /// Create a new [Canvas3d] from an MSAA image and a resolve target. This will allow for drawing with MSAA to a color image, then resolving the samples into a secondary target.
    ///
    /// Both images must be created for Canvas3d usage (see [`Canvas3d::from_image`]). `msaa_image` must have a sample count > 1 and `resolve_image` must strictly have a sample count of 1.
    #[inline]
    pub fn from_msaa(
        gfx: &impl Has<GraphicsContext>,
        msaa_image: Image,
        resolve: Image,
        clear: impl Into<Option<Color>>,
    ) -> Self {
        Canvas3d::new(gfx, msaa_image, Some(resolve), clear.into())
    }

    /// Helper for [`Canvas3d::from_msaa`] for construction of an MSAA [`Canvas3d`] from a [`ScreenImage`].
    #[inline]
    pub fn from_screen_msaa(
        gfx: &impl Has<GraphicsContext>,
        msaa_image: &mut ScreenImage,
        resolve: &mut ScreenImage,
        clear: impl Into<Option<Color>>,
    ) -> Self {
        let msaa = msaa_image.image(gfx);
        let resolve = resolve.image(gfx);
        Canvas3d::from_msaa(gfx, msaa, resolve, clear)
    }

    /// Create a new [Canvas3d] that renders directly to the window surface.
    ///
    /// `clear` will set the image initially to the given color, if a color is provided, or keep it as is, if it's `None`.
    pub fn from_frame(gfx: &impl Has<GraphicsContext>, clear: impl Into<Option<Color>>) -> Self {
        let gfx = gfx.retrieve();
        // these unwraps will never fail
        let samples = gfx.frame_msaa_image.as_ref().unwrap().samples();
        let (target, resolve) = if samples > 1 {
            (
                gfx.frame_msaa_image.clone().unwrap(),
                Some(gfx.frame_image.clone().unwrap()),
            )
        } else {
            (gfx.frame_image.clone().unwrap(), None)
        };
        Canvas3d::new(gfx, target, resolve, clear.into())
    }

    fn new(
        gfx: &impl Has<GraphicsContext>,
        target: Image,
        resolve: Option<Image>,
        clear: Option<Color>,
    ) -> Self {
        let depth = Image::new_depth_canvas_image(gfx, target.width(), target.height(), 1);

        let gfx = gfx.retrieve();

        let state = DrawState3d {
            shader: default_shader(),
            params: None,
            sampler: Sampler::default(),
            projection: glam::Mat4::IDENTITY.into(),
            alpha_mode: AlphaMode::Discard { cutoff: 5 },
            scissor_rect: (0, 0, target.width(), target.height()),
        };

        let screen = Rect {
            x: 0.,
            y: 0.,
            w: target.width() as _,
            h: target.height() as _,
        };

        let mut this = Canvas3d {
            wgpu: gfx.wgpu.clone(),
            draws: vec![],
            state: state.clone(),
            original_state: state,
            screen: Some(screen),
            sort_by: |a, b| a.z.cmp(&b.z),

            target,
            target_depth: depth,
            resolve,
            clear,
            defaults: DefaultResources3d::new(gfx),
        };

        this.set_screen_coordinates(screen);

        this
    }

    /// Sets the shader to use when drawing meshes.
    #[inline]
    pub fn set_shader(&mut self, shader: &Shader) {
        self.state.shader = shader.clone();
    }

    /// Returns the current shader being used when drawing meshes.
    #[inline]
    pub fn shader(&self) -> Shader {
        self.state.shader.clone()
    }

    /// Sets the shader parameters to use when drawing meshes.
    ///
    /// **Bound to bind group 3.**
    #[inline]
    pub fn set_shader_params<Uniforms: AsStd140>(&mut self, params: &ShaderParams<Uniforms>) {
        self.state.params = Some((
            params.bind_group.clone().unwrap(/* always Some */),
            params.layout.clone().unwrap(/* always Some */),
            params.buffer_offset,
        ));
    }

    /// Resets the active mesh shader to the default.
    #[inline]
    pub fn set_default_shader(&mut self) {
        self.state.shader = default_shader();
    }

    /// Sets the active sampler used to sample images.
    ///
    /// Use `set_sampler(Sampler::nearest_clamp())` for drawing pixel art graphics without blurring them.
    #[inline]
    pub fn set_sampler(&mut self, sampler: impl Into<Sampler>) {
        self.state.sampler = sampler.into();
    }

    /// Returns the currently active sampler used to sample images.
    #[inline]
    pub fn sampler(&self) -> Sampler {
        self.state.sampler
    }

    /// Resets the active sampler to the default.
    ///
    /// This is equivalent to `set_sampler(Sampler::linear_clamp())`.
    #[inline]
    pub fn set_default_sampler(&mut self) {
        self.set_sampler(Sampler::default());
    }

    /// Sets the active blend mode used when drawing meshes.
    #[inline]
    pub fn set_blend_mode(&mut self, blend_mode: BlendMode) {
        self.state.alpha_mode = AlphaMode::Blend { blend_mode };
    }
    /// Sets the active blend mode used when drawing meshes. NOTE: This does not currently affect the shader code or pipeline outside of AlphaMode::Depth
    #[inline]
    pub fn set_alpha_mode(&mut self, alpha_mode: AlphaMode) {
        self.state.alpha_mode = alpha_mode;
    }

    /// Returns the currently active blend mode used when drawing meshes.
    #[inline]
    pub fn alpha_mode(&self) -> AlphaMode {
        self.state.alpha_mode
    }

    /// Sets the raw projection matrix to the given homogeneous
    /// This is where you should set your camera matrix at using `Camera3d::calc_matrix()`
    /// transformation matrix.  For an introduction to graphics matrices,
    /// a good source is this: <http://ncase.me/matrix/>
    #[inline]
    pub fn set_projection(&mut self, proj: impl Into<mint::ColumnMatrix4<f32>>) {
        self.state.projection = proj.into();
        self.screen = None;
    }

    /// Gets a copy of the canvas3d's raw projection matrix.
    #[inline]
    pub fn projection(&self) -> mint::ColumnMatrix4<f32> {
        self.state.projection
    }

    /// Premultiplies the given transformation matrix with the current projection matrix.
    pub fn mul_projection(&mut self, transform: impl Into<mint::ColumnMatrix4<f32>>) {
        self.set_projection(
            glam::Mat4::from(transform.into()) * glam::Mat4::from(self.state.projection),
        );
        self.screen = None;
    }

    /// Sets the bounds of the screen viewport. This is a shortcut for `set_projection`
    /// and thus will override any previous projection matrix set.
    ///
    /// The default coordinate system has \[0.0, 0.0\] at the top-left corner
    /// with X increasing to the right and Y increasing down, with the
    /// viewport scaled such that one coordinate unit is one pixel on the
    /// screen.  This function lets you change this coordinate system to
    /// be whatever you prefer.
    ///
    /// The `Rect`'s x and y will define the top-left corner of the screen,
    /// and that plus its w and h will define the bottom-right corner.
    #[inline]
    pub fn set_screen_coordinates(&mut self, rect: Rect) {
        self.set_projection(screen_to_mat(rect));
        self.screen = Some(rect);
    }

    /// Returns the boudns of the screen viewport, iff the projection was last set with
    /// `set_screen_coordinates`. If the last projection was set with `set_projection` or
    /// `mul_projection`, `None` will be returned.
    #[inline]
    pub fn screen_coordinates(&self) -> Option<Rect> {
        self.screen
    }

    /// Sets the scissor rectangle used when drawing. Nothing will be drawn to the canvas
    /// that falls outside of this region.
    ///
    /// Note: The rectangle is in pixel coordinates, and therefore the values will be rounded towards zero.
    #[inline]
    pub fn set_scissor_rect(&mut self, rect: Rect) -> GameResult {
        if rect.w as u32 == 0 || rect.h as u32 == 0 {
            return Err(GameError::RenderError(String::from(
                "the scissor rectangle size must be larger than zero.",
            )));
        }

        let image_size = (self.target.width(), self.target.height());
        if rect.x as u32 >= image_size.0 || rect.y as u32 >= image_size.1 {
            return Err(GameError::RenderError(String::from(
                "the scissor rectangle cannot start outside the canvas image.",
            )));
        }

        // clamp the scissor rectangle to the target image size
        let rect_width = u32::min(image_size.0 - rect.x as u32, rect.w as u32);
        let rect_height = u32::min(image_size.1 - rect.y as u32, rect.h as u32);

        self.state.scissor_rect = (rect.x as u32, rect.y as u32, rect_width, rect_height);

        Ok(())
    }

    /// Returns the scissor rectangle as set by [`Canvas::set_scissor_rect`].
    #[inline]
    pub fn scissor_rect(&self) -> Rect {
        Rect::new(
            self.state.scissor_rect.0 as f32,
            self.state.scissor_rect.1 as f32,
            self.state.scissor_rect.2 as f32,
            self.state.scissor_rect.3 as f32,
        )
    }

    /// Resets the scissorr rectangle back to the original value. This will effectively disable any
    /// scissoring.
    #[inline]
    pub fn set_default_scissor_rect(&mut self) {
        self.state.scissor_rect = self.original_state.scissor_rect;
    }

    /// Sets the sorting function for the final draw order of the canvas.
    /// This is used upon the canvas being finalized (i.e., when [`Canvas3d::finish`] is called).
    /// As such, it is only necessary to call this once per canvas, and will affect *all* draws in this canvas.
    ///
    /// By default, draws will be sorted by the Z index of [`DrawParam3d`].
    #[inline]
    pub fn set_sort_by(&mut self, sort_by: fn(&DrawParam3d, &DrawParam3d) -> Ordering) {
        self.sort_by = sort_by;
    }

    /// Draws the given `Drawable3d` to the canvas with a given `DrawParam3d`.
    #[inline]
    pub fn draw(&mut self, drawable: &impl Drawable3d, param: impl Into<DrawParam3d>) {
        drawable.draw(self, param)
    }

    /// Finish drawing with this canvas and submit all the draw calls.
    #[inline]
    pub fn finish(mut self, gfx: &mut impl HasMut<GraphicsContext>) -> GameResult {
        let gfx = gfx.retrieve_mut();
        self.finalize(gfx)
    }

    #[inline]
    pub(crate) fn push_draw(&mut self, draw: Draw3d, param: DrawParam3d) {
        self.draws.push(DrawCommand3d {
            state: self.state.clone(),
            draw,
            param,
        });
    }

    #[inline]
    pub(crate) fn default_resources(&self) -> &DefaultResources3d {
        &self.defaults
    }

    fn finalize(&mut self, gfx: &mut GraphicsContext) -> GameResult {
        let mut canvas = if let Some(resolve) = &self.resolve {
            InternalCanvas3d::from_msaa(gfx, self.clear, &self.target, &self.target_depth, resolve)?
        } else {
            InternalCanvas3d::from_image(gfx, self.clear, &self.target, &self.target_depth)?
        };

        let mut state = self.state.clone();

        // apply initial state
        canvas.set_shader(state.shader.clone());
        if let Some((bind_group, layout, offset)) = &state.params {
            canvas.set_shader_params(bind_group.clone(), layout.clone(), *offset);
        }

        canvas.set_sampler(state.sampler);
        canvas.set_alpha_mode(state.alpha_mode);
        canvas.set_projection(state.projection);

        if state.scissor_rect.2 > 0 && state.scissor_rect.3 > 0 {
            canvas.set_scissor_rect(state.scissor_rect);
        }

        self.draws
            .sort_by(|a, b| (self.sort_by)(&a.param, &b.param));

        canvas.update_uniform(&self.draws);

        for (idx, draw) in self.draws.iter().enumerate() {
            if draw.state.shader != state.shader {
                canvas.set_shader(draw.state.shader.clone());
            }

            if draw.state.params != state.params {
                if let Some((bind_group, layout, offset)) = &draw.state.params {
                    canvas.set_shader_params(bind_group.clone(), layout.clone(), *offset);
                } else {
                    canvas.reset_shader_params();
                }
            }

            if draw.state.sampler != state.sampler {
                canvas.set_sampler(draw.state.sampler);
            }

            if draw.state.alpha_mode != state.alpha_mode {
                canvas.set_alpha_mode(draw.state.alpha_mode);
            }

            if draw.state.projection != state.projection {
                canvas.set_projection(draw.state.projection);
            }

            if draw.state.scissor_rect != state.scissor_rect {
                canvas.set_scissor_rect(draw.state.scissor_rect);
            }

            state = draw.state.clone();

            match &draw.draw {
                Draw3d::Mesh { mesh } => {
                    if let Some(image) = mesh.texture.clone() {
                        canvas.draw_mesh(mesh, &image, idx)
                    } else {
                        canvas.draw_mesh(mesh, &self.default_resources().image, idx)
                    }
                }
                Draw3d::MeshInstances { mesh, instances } => {
                    canvas.draw_mesh_instances(mesh, instances, draw.param)?
                }
            }
        }

        canvas.finish();

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DrawState3d {
    shader: Shader,
    params: Option<(wgpu::BindGroup, wgpu::BindGroupLayout, u32)>,
    sampler: Sampler,
    pub(crate) projection: mint::ColumnMatrix4<f32>,
    alpha_mode: AlphaMode,
    scissor_rect: (u32, u32, u32, u32),
}

/// Rendered version of mesh slimmed down
#[derive(Clone, Debug)]
pub struct RenderedMesh3d {
    pub(crate) vert_buffer: Arc<wgpu::Buffer>,
    pub(crate) ind_buffer: Arc<wgpu::Buffer>,
    pub(crate) texture: Option<Image>,
    pub(crate) ind_len: usize,
}

impl From<&Mesh3d> for RenderedMesh3d {
    fn from(item: &Mesh3d) -> Self {
        Self {
            vert_buffer: item.vert_buffer.clone(),
            ind_buffer: item.ind_buffer.clone(),
            texture: item.texture.clone(),
            ind_len: item.indices.len(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Draw3d {
    Mesh {
        mesh: RenderedMesh3d,
    },
    MeshInstances {
        mesh: RenderedMesh3d,
        instances: InstanceArrayView3d,
    },
}

// Stores *everything* you need to know to draw something.
#[derive(Debug)]
pub(crate) struct DrawCommand3d {
    pub(crate) state: DrawState3d,
    pub(crate) param: DrawParam3d,
    pub(crate) draw: Draw3d,
}

#[derive(Debug)]
pub(crate) struct DefaultResources3d {
    pub image: Image,
}

impl DefaultResources3d {
    fn new(gfx: &GraphicsContext) -> Self {
        let image = gfx.white_image.clone();

        DefaultResources3d { image }
    }
}
