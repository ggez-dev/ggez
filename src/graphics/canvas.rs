use crevice::std140::AsStd140;

use crate::{
    context::{Has, HasMut},
    GameError, GameResult,
};

use super::{
    gpu::arc::{ArcBindGroup, ArcBindGroupLayout},
    internal_canvas::{screen_to_mat, InstanceArrayView, InternalCanvas},
    BlendMode, Color, DrawParam, Drawable, GraphicsContext, Image, InstanceArray, Mesh, Rect,
    Sampler, ScreenImage, Shader, ShaderParams, Text, WgpuContext, ZIndex,
};
use std::{collections::BTreeMap, sync::Arc};

/// Canvases are the main method of drawing meshes and text to images in ggez.
///
/// They can draw to any image that is capable of being drawn to (i.e. has been created with [`Image::new_canvas_image()`] or [`ScreenImage`]),
/// or they can draw directly to the screen.
///
/// Canvases are also where you can bind your own custom shaders and samplers to use while drawing.
/// Canvases *do not* automatically batch draws. To used batched (instanced) drawing, refer to [`InstanceArray`].
// note:
//   Canvas does not draw anything itself. It is merely a state-tracking and draw-reordering wrapper around InternalCanvas, which does the actual
// drawing.
#[derive(Debug)]
pub struct Canvas {
    pub(crate) wgpu: Arc<WgpuContext>,
    draws: BTreeMap<ZIndex, Vec<DrawCommand>>,
    state: DrawState,
    original_state: DrawState,
    screen: Option<Rect>,
    defaults: DefaultResources,

    target: Image,
    resolve: Option<Image>,
    load_op: CanvasLoadOp,

    // This will be removed after queue_text and draw_queued_text have been removed.
    pub(crate) queued_texts: Vec<(Text, mint::Point2<f32>, Option<Color>)>,
}

impl Canvas {
    /// Create a new [Canvas] from an image. This will allow for drawing to a single color image.
    ///
    /// The image must be created for Canvas usage, i.e. [Image::new_canvas_image()], or [ScreenImage], and must only have a sample count of 1.
    #[inline]
    pub fn from_image(
        gfx: &impl Has<GraphicsContext>,
        image: Image,
        load_op: impl Into<CanvasLoadOp>,
    ) -> Self {
        Canvas::new(gfx, image, None, load_op.into())
    }

    /// Helper for [`Canvas::from_image`] for construction of a [`Canvas`] from a [`ScreenImage`].
    #[inline]
    pub fn from_screen_image(
        gfx: &impl Has<GraphicsContext>,
        image: &mut ScreenImage,
        load_op: impl Into<CanvasLoadOp>,
    ) -> Self {
        let gfx = gfx.retrieve();
        let image = image.image(gfx);
        Canvas::from_image(gfx, image, load_op)
    }

    /// Create a new [Canvas] from an MSAA image and a resolve target. This will allow for drawing with MSAA to a color image, then resolving the samples into a secondary target.
    ///
    /// Both images must be created for Canvas usage (see [Canvas::from_image]). `msaa_image` must have a sample count > 1 and `resolve_image` must strictly have a sample count of 1.
    #[inline]
    pub fn from_msaa(
        gfx: &impl Has<GraphicsContext>,
        msaa_image: Image,
        resolve: Image,
        load_op: impl Into<CanvasLoadOp>,
    ) -> Self {
        Canvas::new(gfx, msaa_image, Some(resolve), load_op.into())
    }

    /// Helper for [`Canvas::from_msaa`] for construction of an MSAA [`Canvas`] from a [`ScreenImage`].
    #[inline]
    pub fn from_screen_msaa(
        gfx: &impl Has<GraphicsContext>,
        msaa_image: &mut ScreenImage,
        resolve: &mut ScreenImage,
        load_op: impl Into<CanvasLoadOp>,
    ) -> Self {
        let msaa = msaa_image.image(gfx);
        let resolve = resolve.image(gfx);
        Canvas::from_msaa(gfx, msaa, resolve, load_op)
    }

    /// Create a new [Canvas] that renders directly to the window surface.
    pub fn from_frame(gfx: &impl Has<GraphicsContext>, load_op: impl Into<CanvasLoadOp>) -> Self {
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
        Canvas::new(gfx, target, resolve, load_op.into())
    }

    fn new(
        gfx: &impl Has<GraphicsContext>,
        target: Image,
        resolve: Option<Image>,
        load_op: CanvasLoadOp,
    ) -> Self {
        let gfx = gfx.retrieve();

        let defaults = DefaultResources::new(gfx);

        let state = DrawState {
            shader: default_shader(),
            params: None,
            text_shader: default_text_shader(),
            text_params: None,
            sampler: default_sampler(),
            blend_mode: BlendMode::ALPHA,
            premul_text: true,
            projection: glam::Mat4::IDENTITY.into(),
            scissor_rect: (0, 0, target.width(), target.height()),
        };

        let screen = Rect {
            x: 0.,
            y: 0.,
            w: target.width() as _,
            h: target.height() as _,
        };

        let mut this = Canvas {
            wgpu: gfx.wgpu.clone(),
            draws: BTreeMap::new(),
            state: state.clone(),
            original_state: state,
            screen: Some(screen),
            defaults,

            target,
            resolve,
            load_op,

            queued_texts: Vec::new(),
        };

        this.set_screen_coordinates(screen);

        this
    }

    /// Sets the shader to use when drawing meshes.
    #[inline]
    pub fn set_shader(&mut self, shader: Shader) {
        self.state.shader = shader;
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
    pub fn set_shader_params<Uniforms: AsStd140>(&mut self, params: ShaderParams<Uniforms>) {
        self.state.params = Some((params.bind_group.clone(), params.layout));
    }

    /// Sets the shader to use when drawing text.
    #[inline]
    pub fn set_text_shader(&mut self, shader: Shader) {
        self.state.text_shader = shader;
    }

    /// Returns the current text shader being used when drawing text.
    #[inline]
    pub fn text_shader(&self) -> Shader {
        self.state.text_shader.clone()
    }

    /// Sets the shader parameters to use when drawing text.
    ///
    /// **Bound to bind group 3.**
    #[inline]
    pub fn set_text_shader_params<Uniforms: AsStd140>(&mut self, params: ShaderParams<Uniforms>) {
        self.state.text_params = Some((params.bind_group.clone(), params.layout));
    }

    /// Resets the active mesh shader to the default.
    #[inline]
    pub fn set_default_shader(&mut self) {
        self.state.shader = default_shader();
    }

    /// Resets the active text shader to the default.
    #[inline]
    pub fn set_default_text_shader(&mut self) {
        self.state.text_shader = default_text_shader();
    }

    /// Sets the active sampler used to sample images.
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
        self.set_sampler(Sampler::linear_clamp());
    }

    /// Sets the active blend mode used when drawing images.
    #[inline]
    pub fn set_blend_mode(&mut self, blend_mode: BlendMode) {
        self.state.blend_mode = blend_mode;
    }

    /// Returns the currently active blend mode used when drawing images.
    #[inline]
    pub fn blend_mode(&self) -> BlendMode {
        self.state.blend_mode
    }

    /// Selects whether text will be drawn with [`BlendMode::PREMULTIPLIED`] when the current blend
    /// mode is [`BlendMode::ALPHA`]. This is `true` by default.
    #[inline]
    pub fn set_premultiplied_text(&mut self, premultiplied_text: bool) {
        self.state.premul_text = premultiplied_text;
    }

    /// Sets the raw projection matrix to the given homogeneous
    /// transformation matrix.  For an introduction to graphics matrices,
    /// a good source is this: <http://ncase.me/matrix/>
    #[inline]
    pub fn set_projection(&mut self, proj: impl Into<mint::ColumnMatrix4<f32>>) {
        self.state.projection = proj.into();
        self.screen = None;
    }

    /// Gets a copy of the canvas's raw projection matrix.
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

    /// Draws the given `Drawable` to the canvas with a given `DrawParam`.
    #[inline]
    pub fn draw(&mut self, drawable: &impl Drawable, param: impl Into<DrawParam>) {
        drawable.draw(self, param)
    }

    /// Draws a `Mesh` textured with an `Image`.
    ///
    /// This differs from `canvas.draw(mesh, param)` as in that case, the mesh is untextured.
    pub fn draw_textured_mesh(&mut self, mesh: Mesh, image: Image, param: impl Into<DrawParam>) {
        self.push_draw(
            Draw::Mesh {
                mesh,
                image,
                scale: false,
            },
            param.into(),
        );
    }

    /// Draws an `InstanceArray` textured with a `Mesh`.
    ///
    /// This differs from `canvas.draw(instances, param)` as in that case, the instances are
    /// drawn as quads.
    pub fn draw_instanced_mesh(
        &mut self,
        mesh: Mesh,
        instances: &InstanceArray,
        param: impl Into<DrawParam>,
    ) {
        instances.flush_wgpu(&self.wgpu).unwrap();
        self.push_draw(
            Draw::MeshInstances {
                mesh,
                instances: InstanceArrayView::from_instances(instances).unwrap(),
                scale: false,
            },
            param.into(),
        );
    }

    /// Finish drawing with this canvas and submit all the draw calls.
    #[inline]
    pub fn finish(mut self, gfx: &mut impl HasMut<GraphicsContext>) -> GameResult {
        let gfx = gfx.retrieve_mut();
        self.finalize(gfx)
    }

    #[inline]
    pub(crate) fn default_resources(&self) -> &DefaultResources {
        &self.defaults
    }

    #[inline]
    pub(crate) fn push_draw(&mut self, draw: Draw, param: DrawParam) {
        self.draws.entry(param.z).or_default().push(DrawCommand {
            state: self.state.clone(),
            draw,
            param,
        });
    }

    fn finalize(&mut self, gfx: &mut GraphicsContext) -> GameResult {
        let mut canvas = if let Some(resolve) = &self.resolve {
            InternalCanvas::from_msaa(gfx, self.load_op, &self.target, resolve)?
        } else {
            InternalCanvas::from_image(gfx, self.load_op, &self.target)?
        };

        let mut state = self.state.clone();

        // apply initial state
        canvas.set_shader(state.shader.clone());
        if let Some((bind_group, layout)) = &state.params {
            canvas.set_shader_params(bind_group.clone(), layout.clone());
        }

        canvas.set_text_shader(state.text_shader.clone());
        if let Some((bind_group, layout)) = &state.text_params {
            canvas.set_text_shader_params(bind_group.clone(), layout.clone());
        }

        canvas.set_sampler(state.sampler);
        canvas.set_blend_mode(state.blend_mode);
        canvas.set_projection(state.projection);

        if state.scissor_rect.2 > 0 && state.scissor_rect.3 > 0 {
            canvas.set_scissor_rect(state.scissor_rect);
        }

        for draws in self.draws.values() {
            for draw in draws {
                // track state and apply to InternalCanvas if changed

                if draw.state.shader != state.shader {
                    canvas.set_shader(draw.state.shader.clone());
                }

                if draw.state.params != state.params {
                    if let Some((bind_group, layout)) = &draw.state.params {
                        canvas.set_shader_params(bind_group.clone(), layout.clone());
                    }
                }

                if draw.state.text_shader != state.text_shader {
                    canvas.set_text_shader(draw.state.text_shader.clone());
                }

                if draw.state.text_params != state.text_params {
                    if let Some((bind_group, layout)) = &draw.state.text_params {
                        canvas.set_text_shader_params(bind_group.clone(), layout.clone());
                    }
                }

                if draw.state.sampler != state.sampler {
                    canvas.set_sampler(draw.state.sampler);
                }

                if draw.state.blend_mode != state.blend_mode {
                    canvas.set_blend_mode(draw.state.blend_mode);
                }

                if draw.state.premul_text != state.premul_text {
                    canvas.set_premultiplied_text(draw.state.premul_text);
                }

                if draw.state.projection != state.projection {
                    canvas.set_projection(draw.state.projection);
                }

                if draw.state.scissor_rect != state.scissor_rect {
                    canvas.set_scissor_rect(draw.state.scissor_rect);
                }

                state = draw.state.clone();

                match &draw.draw {
                    Draw::Mesh { mesh, image, scale } => {
                        canvas.draw_mesh(mesh, image, draw.param, *scale)
                    }
                    Draw::MeshInstances {
                        mesh,
                        instances,
                        scale,
                    } => canvas.draw_mesh_instances(mesh, instances, draw.param, *scale)?,
                    Draw::BoundedText { text } => canvas.draw_bounded_text(text, draw.param)?,
                }
            }
        }

        canvas.finish();

        Ok(())
    }
}

/// Describes the image load operation when starting a new canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanvasLoadOp {
    /// Keep the existing contents of the image.
    DontClear,
    /// Clear the image contents to a solid color.
    Clear(Color),
}

impl From<Option<Color>> for CanvasLoadOp {
    fn from(color: Option<Color>) -> Self {
        match color {
            Some(color) => CanvasLoadOp::Clear(color),
            None => CanvasLoadOp::DontClear,
        }
    }
}

impl From<Color> for CanvasLoadOp {
    #[inline]
    fn from(color: Color) -> Self {
        CanvasLoadOp::Clear(color)
    }
}

#[derive(Debug, Clone)]
struct DrawState {
    shader: Shader,
    params: Option<(ArcBindGroup, ArcBindGroupLayout)>,
    text_shader: Shader,
    text_params: Option<(ArcBindGroup, ArcBindGroupLayout)>,
    sampler: Sampler,
    blend_mode: BlendMode,
    premul_text: bool,
    projection: mint::ColumnMatrix4<f32>,
    scissor_rect: (u32, u32, u32, u32),
}

#[derive(Debug)]
pub(crate) enum Draw {
    Mesh {
        mesh: Mesh,
        image: Image,
        scale: bool,
    },
    MeshInstances {
        mesh: Mesh,
        instances: InstanceArrayView,
        scale: bool,
    },
    BoundedText {
        text: Text,
    },
}

// Stores *everything* you need to know to draw something.
#[derive(Debug)]
struct DrawCommand {
    state: DrawState,
    param: DrawParam,
    draw: Draw,
}

#[derive(Debug)]
pub(crate) struct DefaultResources {
    pub mesh: Mesh,
    pub image: Image,
}

impl DefaultResources {
    fn new(gfx: &GraphicsContext) -> Self {
        let mesh = gfx.rect_mesh.clone();
        let image = gfx.white_image.clone();

        DefaultResources { mesh, image }
    }
}

/// The default shader.
pub fn default_shader() -> Shader {
    Shader {
        fs_module: None,
        vs_module: None,
    }
}

/// The default text shader.
pub fn default_text_shader() -> Shader {
    Shader {
        fs_module: None,
        vs_module: None,
    }
}

/// The default sampler
pub fn default_sampler() -> Sampler {
    Sampler::linear_clamp()
}
