use super::arc::{ArcBindGroupLayout, ArcPipelineLayout, ArcRenderPipeline, ArcShaderModule};
use std::collections::{hash_map::DefaultHasher, HashMap};

/// Hashable representation of a render pipeline, used as a key in the HashMap cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenderPipelineInfo {
    pub layout: ArcPipelineLayout,
    pub vs: ArcShaderModule,
    pub fs: ArcShaderModule,
    pub vs_entry: String,
    pub fs_entry: String,
    pub samples: u32,
    pub format: wgpu::TextureFormat,
    pub blend: Option<wgpu::BlendState>,
    pub depth: Option<wgpu::CompareFunction>,
    pub vertices: bool,
    pub topology: wgpu::PrimitiveTopology,
    pub vertex_layout: wgpu::VertexBufferLayout<'static>,
    pub cull_mode: Option<wgpu::Face>,
}

/// Caches both the pipeline *and* the pipeline layout.
#[derive(Debug)]
pub struct PipelineCache {
    pipelines: HashMap<RenderPipelineInfo, ArcRenderPipeline>,
    layouts: HashMap<u64, ArcPipelineLayout>,
}

impl PipelineCache {
    pub fn new() -> Self {
        PipelineCache {
            pipelines: HashMap::new(),
            layouts: HashMap::new(),
        }
    }

    pub fn render_pipeline(
        &mut self,
        device: &wgpu::Device,
        info: RenderPipelineInfo,
    ) -> ArcRenderPipeline {
        let vertex_buffers = [info.vertex_layout.clone()];

        self.pipelines
            .entry(info)
            .or_insert_with_key(|info| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: None,
                    layout: Some(&info.layout),
                    vertex: wgpu::VertexState {
                        module: &info.vs,
                        entry_point: Some(&info.vs_entry),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: if info.vertices { &vertex_buffers } else { &[] },
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: info.topology,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: info.cull_mode,
                        unclipped_depth: false,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        conservative: false,
                    },
                    depth_stencil: info.depth.map(|depth_compare| wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: true,
                        depth_compare,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: info.samples,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &info.fs,
                        entry_point: Some(&info.fs_entry),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: info.format,
                            blend: info.blend,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview: None,
                    cache: None,
                })
            })
            .clone()
    }

    pub fn layout(
        &mut self,
        device: &wgpu::Device,
        bind_groups: &[ArcBindGroupLayout],
    ) -> ArcPipelineLayout {
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            for bg in bind_groups {
                bg.hash(&mut h);
            }
            h.finish()
        };
        self.layouts
            .entry(key)
            .or_insert_with(|| {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &bind_groups.iter().collect::<Vec<_>>(),
                    push_constant_ranges: &[],
                })
            })
            .clone()
    }
}
