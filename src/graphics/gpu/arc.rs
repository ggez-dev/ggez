use std::sync::Arc;

pub type ArcBuffer = wgpu::Buffer;
pub type ArcTexture = wgpu::Texture;
pub type ArcTextureView = wgpu::TextureView;
pub type ArcBindGroupLayout = wgpu::BindGroupLayout;
pub type ArcBindGroup = wgpu::BindGroup;
pub type ArcPipelineLayout = wgpu::PipelineLayout;
pub type ArcRenderPipeline = wgpu::RenderPipeline;
pub type ArcSampler = wgpu::Sampler;
pub type ArcShaderModule = Arc<wgpu::ShaderModule>;
