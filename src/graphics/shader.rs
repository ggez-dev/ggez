use std::marker::PhantomData;
use std::{io::Read, sync::Arc};

use crate::{context::Has, Context, GameError, GameResult};

use super::{
    context::GraphicsContext,
    gpu::{bind_group::BindGroupBuilder, growing::GrowingBufferArena},
    image::Image,
    sampler::Sampler,
};
use crevice::std140::Std140;

#[derive(Debug, PartialEq, Eq)]
enum ShaderSource<'a> {
    None,
    Path(&'a str),
    Code(&'a str),
}

/// Builder pattern for assembling shaders.
#[derive(Debug)]
pub struct ShaderBuilder<'a> {
    fs: ShaderSource<'a>,
    vs: ShaderSource<'a>,
}

impl<'a> ShaderBuilder<'a> {
    /// Create a new builder with no associated shader code.
    pub fn new() -> Self {
        ShaderBuilder {
            fs: ShaderSource::None,
            vs: ShaderSource::None,
        }
    }

    /// Use this wgsl code as both a vertex and fragment shader.
    pub fn from_code(source: &'a str) -> Self {
        ShaderBuilder {
            fs: ShaderSource::Code(source),
            vs: ShaderSource::Code(source),
        }
    }

    /// Use a single wgsl resource as both a vertex and fragment shader.
    pub fn from_path(path: &'a str) -> Self {
        ShaderBuilder {
            fs: ShaderSource::Path(path),
            vs: ShaderSource::Path(path),
        }
    }

    /// Use this wgsl shader code for the fragment shader.
    #[must_use]
    pub fn fragment_code(self, source: &'a str) -> Self {
        ShaderBuilder {
            fs: ShaderSource::Code(source),
            vs: self.vs,
        }
    }
    /// Use this wgsl code resource path for the fragment shader.
    #[must_use]
    pub fn fragment_path(self, path: &'a str) -> Self {
        ShaderBuilder {
            fs: ShaderSource::Path(path),
            vs: self.vs,
        }
    }

    /// Use this wgsl shader code for the vertex shader.
    #[must_use]
    pub fn vertex_code(self, source: &'a str) -> Self {
        ShaderBuilder {
            fs: self.vs,
            vs: ShaderSource::Code(source),
        }
    }

    /// Use this wgsl code resource path for the vertex shader.
    #[must_use]
    pub fn vertex_path(self, path: &'a str) -> Self {
        ShaderBuilder {
            fs: self.vs,
            vs: ShaderSource::Path(path),
        }
    }

    /// Create a Shader from the builder.
    pub fn build(self, gfx: &impl Has<GraphicsContext>) -> GameResult<Shader> {
        let gfx = gfx.retrieve();
        let load = |s: &str| {
            Some(Arc::new(gfx.wgpu.device.create_shader_module(
                wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(s.into()),
                },
            )))
        };
        let load_resource = |path: &str| -> GameResult<Option<Arc<wgpu::ShaderModule>>> {
            let mut encoded = Vec::new();
            _ = gfx.fs.open(path)?.read_to_end(&mut encoded)?;
            Ok(load(
                &String::from_utf8(encoded).map_err(GameError::ShaderEncodingError)?,
            ))
        };
        let load_any = |source| -> GameResult<Option<Arc<wgpu::ShaderModule>>> {
            Ok(match source {
                ShaderSource::Code(source) => load(source),
                ShaderSource::Path(source) => load_resource(source)?,
                ShaderSource::None => None,
            })
        };
        Ok(if self.vs == self.fs {
            let module = load_any(self.vs)?;
            Shader {
                vs_module: module.clone(),
                fs_module: module,
            }
        } else {
            Shader {
                vs_module: load_any(self.vs)?,
                fs_module: load_any(self.fs)?,
            }
        })
    }
}

impl Default for ShaderBuilder<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// A custom shader that can be used to render with shader effects.
///
/// The shader may have a user specified vertex module, fragment module, both,
/// or neither. The fragment module entry point must be named `fs_main`. The
/// vertex module entry point must be named `vs_main`. The vertex module must
/// have an output of type
/// ```wgsl
/// struct VertexOutput {
///     @builtin(position) position: vec4<f32>,
///     @location(0) uv: vec2<f32>,
///     @location(1) color: vec4<f32>,
/// }
/// ```
/// if the fragment module is left unspecified (default).
///
/// Produce a Shader using [`ShaderBuilder`].
///
/// Adapted from the `shader.rs` example:
/// ```rust
/// use ggez::*;
/// use ggez::graphics::*;
/// use crevice::std140::AsStd140;
///
/// #[derive(AsStd140)]
/// struct Dim {
///     rate: f32,
/// }
///
/// struct MainState {}
///
/// impl event::EventHandler for MainState {
/// #   fn update(&mut self, _ctx: &mut Context) -> GameResult { Ok(()) }
///     fn draw(&mut self, ctx: &mut Context) -> GameResult {
///         let mut canvas = graphics::Canvas::from_frame(ctx, Color::BLACK);
///         let dim = Dim { rate: 0.5 };
///         // NOTE: This is for simplicity; do not recreate your shader every frame like this!
///         //       For more info look at the full example.
///         let shader = ShaderBuilder::new()
///             .fragment_code(include_str!("../../resources/dimmer.wgsl"))
///             .build(&mut ctx.gfx)?;
///         let mut params = ShaderParamsBuilder::new(&dim).build(ctx);
///         params.set_uniforms(ctx, &dim);
///
///         canvas.set_shader(&shader);
///         canvas.set_shader_params(&params);
///         // draw something...
///         canvas.finish(ctx)
///     }
///
///     /* ... */
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shader {
    pub(crate) vs_module: Option<Arc<wgpu::ShaderModule>>,
    pub(crate) fs_module: Option<Arc<wgpu::ShaderModule>>,
}

impl Shader {
    /// Get the underlying vertex shader module for wgpu
    pub fn vs_module(&self) -> Option<&wgpu::ShaderModule> {
        if let Some(vs_module) = &self.vs_module {
            Some(vs_module)
        } else {
            None
        }
    }

    /// Get the underlying fragment shader module for wgpu
    pub fn fs_module(&self) -> Option<&wgpu::ShaderModule> {
        if let Some(fs_module) = &self.fs_module {
            Some(fs_module)
        } else {
            None
        }
    }
}

use crevice::std140::AsStd140;

/// A builder for [`ShaderParams`]
#[derive(Debug)]
pub struct ShaderParamsBuilder<'a, Uniforms: AsStd140> {
    uniforms: &'a Uniforms,
    images: &'a [&'a Image],
    samplers: &'a [Sampler],
    images_vs_visible: bool,
}

impl<'a, Uniforms: AsStd140> ShaderParamsBuilder<'a, Uniforms> {
    /// Creates a new builder for [`ShaderParams`].
    ///
    /// # Arguments
    ///
    /// * `uniforms` - Initial uniforms.
    pub fn new(uniforms: &'a Uniforms) -> Self {
        ShaderParamsBuilder {
            uniforms,
            images: &[],
            samplers: &[],
            images_vs_visible: false,
        }
    }

    /// Provides images to the shaders.
    ///
    /// # Arguments
    ///
    /// * `vs_visible` - If the images should also be visible to the vertex shader, rather
    ///    than just the fragment shader.
    #[must_use]
    pub fn images(
        self,
        images: &'a [&'a Image],
        samplers: &'a [Sampler],
        vs_visible: bool,
    ) -> Self {
        ShaderParamsBuilder {
            uniforms: self.uniforms,
            images,
            samplers,
            images_vs_visible: vs_visible,
        }
    }

    /// Produce a [`ShaderParams`] from the builder.
    pub fn build(self, ctx: &mut Context) -> ShaderParams<Uniforms> {
        let images = self.images.iter().map(|image| image.view.clone()).collect();
        let samplers = self
            .samplers
            .iter()
            .map(|&sampler| ctx.gfx.sampler_cache.get(&ctx.gfx.wgpu.device, sampler))
            .collect();

        let mut params = ShaderParams {
            uniform_arena: GrowingBufferArena::new(
                &ctx.gfx.wgpu.device,
                u64::from(
                    ctx.gfx
                        .wgpu
                        .device
                        .limits()
                        .min_uniform_buffer_offset_alignment,
                ),
                wgpu::BufferDescriptor {
                    label: None,
                    size: ShaderParams::<Uniforms>::UPDATES_PER_ARENA
                        * Uniforms::std140_size_static() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            ),
            layout: None,
            bind_group: None,
            buffer_offset: 0,
            images,
            samplers,
            images_vs_visible: self.images_vs_visible,
            last_tick: 0,
            _marker: PhantomData,
        };
        params.set_uniforms(ctx, self.uniforms);
        params
    }
}

/// Parameters that can be passed to a custom shader, including uniforms, images, and samplers.
///
/// Create with [`ShaderParamsBuilder`].
///
/// These parameters are bound to group 3. With WGSL, for example,
/// ```rust,ignore
/// ggez::graphics::ShaderParamsBuilder::new(&my_uniforms)
///     .images(&[&image1, &image2], &[sampler1], false)
///     .build(&mut ctx.gfx)
/// ```
/// Corresponds to...
/// ```ignore
/// @group(3) @binding(0)
/// var<uniform> my_uniforms: MyUniforms;
/// @group(3) @binding(1)
/// var image1: texture_2d<f32>;
/// @group(3) @binding(2)
/// var image2: texture_2d<f32>;
/// @group(3) @binding(3)
/// var sampler1: sampler;
/// ```
#[derive(Debug)]
pub struct ShaderParams<Uniforms: AsStd140> {
    uniform_arena: GrowingBufferArena,
    // layout and bind_group always Some after construction
    pub(crate) layout: Option<wgpu::BindGroupLayout>,
    pub(crate) bind_group: Option<wgpu::BindGroup>,
    pub(crate) buffer_offset: u32,
    images: Vec<wgpu::TextureView>,
    samplers: Vec<wgpu::Sampler>,
    images_vs_visible: bool,
    last_tick: usize,
    _marker: PhantomData<Uniforms>,
}

impl<Uniforms: AsStd140> ShaderParams<Uniforms> {
    // this is how many times the uniforms can be updated in one frame before a new buffer needs to be allocated.
    // this is preemptive - if the user never updates then this is a waste, but if the user updates very often in any given frame then we'll have too many buffers + bind groups
    // therefore, TODO: make this number user customizable?
    const UPDATES_PER_ARENA: u64 = 16;

    /// Updates the uniform data.
    ///
    /// When called, [`Canvas::set_shader_params`] (or [`Canvas::set_text_shader_params`]) **needs to be called again** for the new uniforms to take effect.
    pub fn set_uniforms(&mut self, ctx: &mut Context, uniforms: &Uniforms) {
        if ctx.time.ticks() != self.last_tick {
            self.uniform_arena.free();
            self.last_tick = ctx.time.ticks();
        }
        let alloc = self
            .uniform_arena
            .allocate(&ctx.gfx.wgpu.device, Uniforms::std140_size_static() as u64);
        ctx.gfx.wgpu.queue.write_buffer(
            &alloc.buffer,
            alloc.offset,
            uniforms.as_std140().as_bytes(),
        );

        self.buffer_offset = alloc.offset as u32;

        let mut builder = BindGroupBuilder::new();
        builder = builder.buffer(
            &alloc.buffer,
            0,
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            wgpu::BufferBindingType::Uniform,
            true,
            None,
        );

        let vis = if self.images_vs_visible {
            wgpu::ShaderStages::VERTEX_FRAGMENT
        } else {
            wgpu::ShaderStages::FRAGMENT
        };

        for view in &self.images {
            builder = builder.image(view, vis);
        }

        for sampler in &self.samplers {
            builder = builder.sampler(sampler, vis);
        }

        let (bind_group, layout) =
            builder.create(&ctx.gfx.wgpu.device, &mut ctx.gfx.bind_group_cache);
        self.layout = Some(layout);
        self.bind_group = Some(bind_group);
    }
}

pub use wgpu::{BlendComponent, BlendFactor, BlendOperation};

/// Describes the blend mode used when drawing images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlendMode {
    /// The blend mode for the color channels.
    pub color: BlendComponent,
    /// The blend mode for the alpha channel.
    pub alpha: BlendComponent,
}

impl BlendMode {
    /// When combining two fragments, add their values together, saturating
    /// at 1.0
    pub const ADD: Self = BlendMode {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::OneMinusDstAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    };

    /// When combining two fragments, subtract the source value from the
    /// destination value
    pub const SUBTRACT: Self = BlendMode {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::ReverseSubtract,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    };

    /// When combining two fragments, add the value of the source times its
    /// alpha channel with the value of the destination multiplied by the inverse
    /// of the source alpha channel. Has the usual transparency effect: mixes the
    /// two colors using a fraction of each one specified by the alpha of the source.
    pub const ALPHA: Self = BlendMode {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::OneMinusDstAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    };

    /// When combining two fragments, subtract the destination color from a constant
    /// color using the source color as weight. Has an invert effect with the constant
    /// color as base and source color controlling displacement from the base color.
    /// A white source color and a white value results in plain invert. The output
    /// alpha is same as destination alpha.
    pub const INVERT: Self = BlendMode {
        color: BlendComponent {
            src_factor: BlendFactor::Constant,
            dst_factor: BlendFactor::Src,
            operation: BlendOperation::Subtract,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    };

    /// When combining two fragments, multiply their values together (including alpha)
    pub const MULTIPLY: Self = BlendMode {
        color: BlendComponent {
            src_factor: BlendFactor::Dst,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::DstAlpha,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
    };

    /// When combining two fragments, choose the source value (including source alpha)
    pub const REPLACE: Self = BlendMode {
        color: wgpu::BlendState::REPLACE.color,
        alpha: wgpu::BlendState::REPLACE.alpha,
    };

    /// When combining two fragments, choose the lighter value
    pub const LIGHTEN: Self = BlendMode {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Max,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::OneMinusDstAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    };

    /// When combining two fragments, choose the darker value
    pub const DARKEN: Self = BlendMode {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Min,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::OneMinusDstAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    };

    /// When using premultiplied alpha, use this.
    ///
    /// You usually want to use this blend mode for drawing canvases
    /// containing semi-transparent imagery.
    /// For an explanation on this see: <https://github.com/ggez/ggez/issues/694#issuecomment-853724926>
    pub const PREMULTIPLIED: Self = BlendMode {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::OneMinusDstAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    };
}
