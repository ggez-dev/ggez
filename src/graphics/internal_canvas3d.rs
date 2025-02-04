use super::{
    context::GraphicsContext,
    gpu::{
        bind_group::{BindGroupBuilder, BindGroupCache, BindGroupLayoutBuilder},
        growing::{ArenaAllocation, GrowingBufferArena},
        pipeline::{PipelineCache, RenderPipelineInfo},
    },
    image::Image,
    instance3d::InstanceArray3d,
    sampler::{Sampler, SamplerCache},
    shader::Shader,
    AlphaMode, BlendMode, Color, Draw3d, DrawCommand3d, DrawParam3d, DrawUniforms3d, LinearColor,
    Rect, RenderedMesh3d, Vertex3d, WgpuContext,
};
use crate::{GameError, GameResult};
use crevice::std140::AsStd140;
use glam::{Mat4, Vec4};
use std::hash::Hash;

/// A canvas represents a render pass and is how you render meshes .
#[allow(missing_debug_implementations)]
pub struct InternalCanvas3d<'a> {
    wgpu: &'a WgpuContext,
    bind_group_cache: &'a mut BindGroupCache,
    pipeline_cache: &'a mut PipelineCache,
    sampler_cache: &'a mut SamplerCache,
    uniform_arena: &'a mut GrowingBufferArena,

    shader: Shader,
    shader_bind_group: Option<(wgpu::BindGroup, wgpu::BindGroupLayout, u32)>,

    shader_ty: Option<ShaderType3d>,
    dirty_pipeline: bool,
    alpha_mode: AlphaMode,
    blend_mode: BlendMode,
    pass: wgpu::RenderPass<'a>,
    samples: u32,
    format: wgpu::TextureFormat,

    draw_sm: &'a wgpu::ShaderModule,
    instance_sm: &'a wgpu::ShaderModule,
    instance_unordered_sm: &'a wgpu::ShaderModule,

    transform: glam::Mat4,
    curr_image: Option<wgpu::TextureView>,
    curr_sampler: Sampler,
    next_sampler: Sampler,

    uniform_alloc: Option<ArenaAllocation>,
}

impl<'a> InternalCanvas3d<'a> {
    pub fn from_image(
        gfx: &'a mut GraphicsContext,
        clear: impl Into<Option<Color>>,
        image: &'a Image,
        depth: &'a Image,
    ) -> GameResult<Self> {
        if image.samples() > 1 {
            return Err(GameError::RenderError(String::from("non-MSAA rendering requires an image with exactly 1 sample, for this image use Canvas::from_msaa instead")));
        }

        Self::new(gfx, 1, image.format(), |cmd| {
            cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &image.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: match clear.into() {
                            None => wgpu::LoadOp::Load,
                            Some(color) => wgpu::LoadOp::Clear(LinearColor::from(color).into()),
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth.wgpu().1,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            })
        })
    }

    pub fn from_msaa(
        gfx: &'a mut GraphicsContext,
        clear: impl Into<Option<Color>>,
        msaa_image: &'a Image,
        depth: &'a Image,
        resolve_image: &'a Image,
    ) -> GameResult<Self> {
        if msaa_image.samples() == 1 {
            return Err(GameError::RenderError(String::from(
                "MSAA rendering requires an image with more than 1 sample, for this image use Canvas::from_image instead",
            )));
        }

        if resolve_image.samples() > 1 {
            return Err(GameError::RenderError(String::from(
                "can only resolve into an image with exactly 1 sample",
            )));
        }

        if msaa_image.format() != resolve_image.format() {
            return Err(GameError::RenderError(String::from(
                "MSAA image and resolve image must be the same format",
            )));
        }

        Self::new(gfx, msaa_image.samples(), msaa_image.format(), |cmd| {
            cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa_image.view,
                    resolve_target: Some(&resolve_image.view),
                    ops: wgpu::Operations {
                        load: match clear.into() {
                            None => wgpu::LoadOp::Load,
                            Some(color) => wgpu::LoadOp::Clear(LinearColor::from(color).into()),
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth.wgpu().1,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            })
        })
    }

    pub(crate) fn new(
        gfx: &'a mut GraphicsContext,
        samples: u32,
        format: wgpu::TextureFormat,
        create_pass: impl FnOnce(&'a mut wgpu::CommandEncoder) -> wgpu::RenderPass<'a>,
    ) -> GameResult<Self> {
        if gfx.fcx.is_none() {
            return Err(GameError::RenderError(String::from(
                "starting Canvas outside of a frame",
            )));
        }

        let drawable_size = gfx.drawable_size();

        let wgpu = &gfx.wgpu;
        let bind_group_cache = &mut gfx.bind_group_cache;
        let pipeline_cache = &mut gfx.pipeline_cache;
        let sampler_cache = &mut gfx.sampler_cache;
        let uniform_arena = &mut gfx.uniform_arena;

        let mut pass = {
            let fcx = gfx.fcx.as_mut().unwrap(/* see above */);
            create_pass(&mut fcx.cmd)
        };

        pass.set_blend_constant(wgpu::Color::BLACK);

        let screen_coords = Rect {
            x: 0.,
            y: 0.,
            w: drawable_size.0 as _,
            h: drawable_size.1 as _,
        };
        let transform = screen_to_mat(screen_coords);

        let shader = Shader {
            vs_module: None,
            fs_module: None,
        };

        Ok(InternalCanvas3d {
            wgpu,
            bind_group_cache,
            pipeline_cache,
            sampler_cache,
            uniform_arena,

            shader,
            shader_bind_group: None,

            shader_ty: None,
            dirty_pipeline: true,
            alpha_mode: AlphaMode::Discard { cutoff: 5 },
            pass,
            samples,
            format,

            draw_sm: &gfx.draw_shader_3d,
            instance_sm: &gfx.instance_shader_3d,
            instance_unordered_sm: &gfx.instance_unordered_shader_3d,

            transform,
            curr_image: None,
            curr_sampler: Sampler::default(),
            next_sampler: Sampler::default(),
            blend_mode: BlendMode::ALPHA,

            uniform_alloc: None,
        })
    }

    pub fn set_shader_params(
        &mut self,
        bind_group: wgpu::BindGroup,
        layout: wgpu::BindGroupLayout,
        offset: u32,
    ) {
        self.dirty_pipeline = true;
        self.shader_bind_group = Some((bind_group, layout, offset));
    }

    pub fn reset_shader_params(&mut self) {
        self.dirty_pipeline = true;
        self.shader_bind_group = None;
    }

    pub fn set_shader(&mut self, shader: Shader) {
        self.dirty_pipeline = true;
        self.shader = shader;
    }

    pub fn set_sampler(&mut self, sampler: Sampler) {
        self.next_sampler = sampler;
    }

    pub fn set_alpha_mode(&mut self, alpha_mode: AlphaMode) {
        self.dirty_pipeline = true;
        self.alpha_mode = alpha_mode;
    }

    pub fn set_projection(&mut self, proj: impl Into<mint::ColumnMatrix4<f32>>) {
        self.transform = proj.into().into();
    }

    pub fn set_scissor_rect(&mut self, (x, y, w, h): (u32, u32, u32, u32)) {
        self.pass.set_scissor_rect(x, y, w, h);
    }

    pub(crate) fn update_uniform(&mut self, draws: &[DrawCommand3d]) {
        let alignment = self
            .wgpu
            .device
            .limits()
            .min_uniform_buffer_offset_alignment as u64;
        let mut alloc_size = 0;
        let mut uniforms = Vec::new();
        for draw in draws {
            if draw.state.projection != self.transform.into() {
                self.set_projection(draw.state.projection);
            }
            if let Draw3d::Mesh { .. } = &draw.draw {
                alloc_size += alignment;
                let draw_uniform =
                    DrawUniforms3d::from_param(&draw.param).projection(self.transform);
                let mut bytes = draw_uniform.as_std140().as_bytes().to_vec();
                let needed_padding = alignment - (bytes.len() as u64 % alignment); // Pad the uniforms so we can index properly
                bytes.resize(bytes.len() + needed_padding as usize, 0);
                uniforms.extend_from_slice(bytes.as_slice());
            }
        }

        let uniform_alloc = self.uniform_arena.allocate(&self.wgpu.device, alloc_size);
        self.wgpu.queue.write_buffer(
            &uniform_alloc.buffer,
            uniform_alloc.offset,
            uniforms.as_slice(),
        );

        self.uniform_alloc = Some(uniform_alloc);
    }

    pub fn draw_mesh(&mut self, mesh: &'a RenderedMesh3d, image: &Image, idx: usize) {
        self.update_pipeline(ShaderType3d::Draw);

        let (uniform_bind_group, _) = BindGroupBuilder::new()
            .buffer(
                &self.uniform_alloc.as_ref().unwrap().buffer,
                0,
                wgpu::ShaderStages::VERTEX,
                wgpu::BufferBindingType::Uniform,
                true,
                Some(DrawUniforms3d::std140_size_static() as u64),
            )
            .create(&self.wgpu.device, self.bind_group_cache);

        self.set_image(image.clone());

        // 1. allocate some uniform buffer memory from GrowingBufferArena.
        // 2. write the uniform data to that memory
        // 3. use a "dynamic offset" to offset into the memory

        let offset = self.uniform_alloc.as_ref().unwrap().offset + (idx as u64 * 256);

        self.pass.set_bind_group(
            0,
            &uniform_bind_group,
            &[offset as u32], // <- the dynamic offset
        );

        self.pass.set_vertex_buffer(0, mesh.vert_buffer.slice(..));
        self.pass
            .set_index_buffer(mesh.ind_buffer.slice(..), wgpu::IndexFormat::Uint32);

        self.pass.draw_indexed(0..mesh.ind_len as _, 0, 0..1);
    }

    pub fn draw_mesh_instances(
        &mut self,
        mesh: &'a RenderedMesh3d,
        instances: &'a InstanceArrayView3d,
        param: DrawParam3d,
    ) -> GameResult {
        if instances.len == 0 {
            return Ok(());
        }

        self.update_pipeline(ShaderType3d::Instance {
            ordered: instances.ordered,
        });

        let alloc_size = u64::from(
            self.wgpu
                .device
                .limits()
                .min_uniform_buffer_offset_alignment,
        );
        let uniform_alloc = self.uniform_arena.allocate(&self.wgpu.device, alloc_size);

        let (uniform_bind_group, _) = BindGroupBuilder::new()
            .buffer(
                &uniform_alloc.buffer,
                0,
                wgpu::ShaderStages::VERTEX,
                wgpu::BufferBindingType::Uniform,
                true,
                Some(alloc_size),
            )
            .create(&self.wgpu.device, self.bind_group_cache);

        self.set_image(instances.image.clone());
        let draw_uniforms = DrawUniforms3d::from_param(&param).projection(self.transform);
        let uniforms = InstanceUniforms3d {
            model_transform: draw_uniforms.model_transform,
            camera_transform: draw_uniforms.camera_transform,
            color: glam::Vec4::from_array(param.color.into()),
        };

        self.wgpu.queue.write_buffer(
            &uniform_alloc.buffer,
            uniform_alloc.offset,
            uniforms.as_std140().as_bytes(),
        );

        self.pass
            .set_bind_group(0, &uniform_bind_group, &[uniform_alloc.offset as u32]);
        self.pass.set_bind_group(2, &instances.bind_group, &[]);

        self.pass.set_vertex_buffer(0, mesh.vert_buffer.slice(..)); // These buffers should always exist if I recall correctly
        self.pass
            .set_index_buffer(mesh.ind_buffer.slice(..), wgpu::IndexFormat::Uint32);

        self.pass
            .draw_indexed(0..mesh.ind_len as _, 0, 0..instances.len as _);

        Ok(())
    }

    pub fn finish(mut self) {
        self.finalize();
    }

    fn finalize(&mut self) {}

    fn update_pipeline(&mut self, ty: ShaderType3d) {
        if self.dirty_pipeline || self.shader_ty != Some(ty) {
            self.dirty_pipeline = false;
            self.shader_ty = Some(ty);

            let texture_layout = BindGroupLayoutBuilder::new()
                .image(wgpu::ShaderStages::FRAGMENT)
                .sampler(wgpu::ShaderStages::FRAGMENT)
                .create(&self.wgpu.device, self.bind_group_cache);

            let instance_layout = BindGroupLayoutBuilder::new()
                .buffer(
                    wgpu::ShaderStages::VERTEX,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    false,
                )
                .buffer(
                    wgpu::ShaderStages::VERTEX,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    false,
                )
                .create(&self.wgpu.device, self.bind_group_cache);

            let uniform_layout = BindGroupLayoutBuilder::new()
                .seed(ty)
                .buffer(
                    wgpu::ShaderStages::VERTEX,
                    wgpu::BufferBindingType::Uniform,
                    true,
                )
                .create(&self.wgpu.device, self.bind_group_cache);

            let (dummy_group, dummy_layout) =
                BindGroupBuilder::new().create(&self.wgpu.device, self.bind_group_cache);

            let mut groups = vec![&uniform_layout, &texture_layout];

            if let ShaderType3d::Instance { .. } = ty {
                groups.push(&instance_layout);
            } else {
                // the dummy group ensures the user's bind group is at index 3
                groups.push(&dummy_layout);
                self.pass.set_bind_group(2, &dummy_group, &[]);
            }

            let shader = match ty {
                ShaderType3d::Draw | ShaderType3d::Instance { .. } => {
                    if let Some((ref bind_group, ref bind_group_layout, offset)) =
                        self.shader_bind_group
                    {
                        self.pass.set_bind_group(3, bind_group, &[offset]);
                        groups.push(bind_group_layout);
                    }

                    &self.shader
                }
            };

            let layout = self.pipeline_cache.layout(&self.wgpu.device, &groups);
            let pipeline = self.pipeline_cache.render_pipeline(
                &self.wgpu.device,
                RenderPipelineInfo {
                    layout,
                    vs: if let Some(vs_module) = &shader.vs_module {
                        vs_module.clone()
                    } else {
                        match ty {
                            ShaderType3d::Draw => self.draw_sm.clone(),
                            ShaderType3d::Instance { ordered } => {
                                if ordered {
                                    self.instance_sm.clone()
                                } else {
                                    self.instance_unordered_sm.clone()
                                }
                            }
                        }
                    },
                    fs: if let Some(fs_module) = &shader.fs_module {
                        fs_module.clone()
                    } else {
                        match ty {
                            ShaderType3d::Draw | ShaderType3d::Instance { .. } => {
                                self.draw_sm.clone()
                            }
                        }
                    },
                    vs_entry: "vs_main",
                    fs_entry: "fs_main",
                    samples: self.samples,
                    format: self.format,
                    blend: Some(wgpu::BlendState {
                        color: self.blend_mode.color,
                        alpha: self.blend_mode.alpha,
                    }),
                    depth: Some(wgpu::CompareFunction::Less),
                    vertices: true,
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    vertex_layout: Vertex3d::desc(),
                    cull_mode: Some(wgpu::Face::Back),
                },
            );

            self.pass.set_pipeline(&pipeline);
        }
    }

    fn set_image(&mut self, image: Image) {
        if self.curr_sampler != self.next_sampler
            || self
                .curr_image
                .as_ref()
                .map_or(true, |curr| *curr != image.view)
        {
            self.curr_sampler = self.next_sampler;
            let sample = self.sampler_cache.get(&self.wgpu.device, self.curr_sampler);
            let image_bind = image.fetch_buffer(sample, &self.wgpu.device);

            self.curr_image = Some(image.view);

            self.pass.set_bind_group(1, &image_bind, &[]);
        }
    }
}

impl Drop for InternalCanvas3d<'_> {
    fn drop(&mut self) {
        self.finalize();
    }
}

#[derive(Debug)]
pub struct InstanceArrayView3d {
    pub bind_group: wgpu::BindGroup,
    pub image: Image,
    pub len: u32,
    pub ordered: bool,
}

impl InstanceArrayView3d {
    pub fn from_instances(ia: &InstanceArray3d) -> GameResult<Self> {
        Ok(InstanceArrayView3d {
            bind_group: ia
                .bind_group
                .lock()
                .map_err(|_| GameError::LockError)?
                .clone(),
            image: ia.image.clone(),
            len: ia.instances().len() as u32,
            ordered: ia.ordered,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum ShaderType3d {
    Draw,
    Instance { ordered: bool },
}

#[derive(crevice::std140::AsStd140)]
struct InstanceUniforms3d {
    pub color: Vec4,
    pub model_transform: Mat4,
    pub camera_transform: Mat4,
}

pub(crate) fn screen_to_mat(screen: Rect) -> glam::Mat4 {
    glam::Mat4::orthographic_rh(
        screen.left(),
        screen.right(),
        screen.bottom(),
        screen.top(),
        0.,
        1.,
    )
}
