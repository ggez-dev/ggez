use super::{
    arc::{ArcBindGroup, ArcBindGroupLayout, ArcBuffer, ArcTexture, ArcTextureView},
    bind_group::BindGroupBuilder,
    growing::GrowingBufferArena,
};
use crate::graphics::{context::FrameArenas, LinearColor};
use glyph_brush::{GlyphBrush, GlyphBrushBuilder};
use ordered_float::OrderedFloat;
use std::cell::RefCell;

pub(crate) struct TextRenderer {
    // RefCell to make various getter not take &mut.
    pub glyph_brush: RefCell<GlyphBrush<TextVertex, Extra>>,
    pub cache: ArcTexture,
    pub cache_view: ArcTextureView,
    pub cache_bind: ArcBindGroup,
    pub cache_bind_layout: ArcBindGroupLayout,
    pub cache_size: (u32, u32),

    pub verts: GrowingBufferArena,
}

impl TextRenderer {
    // if the number of chars goes over this, a dedicated buffer is allocated for the text
    const MAX_TEXT_VERTEX_ARENA: u64 = 2048;

    pub fn new(device: &wgpu::Device, cache_bind_layout: ArcBindGroupLayout) -> Self {
        let cache_size = (1024, 1024);

        let glyph_brush = GlyphBrushBuilder::using_fonts(vec![])
            .cache_redraws(false)
            .initial_cache_size(cache_size)
            .build();

        let cache = ArcTexture::new(device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: cache_size.0,
                height: cache_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        }));

        let cache_view =
            ArcTextureView::new(cache.create_view(&wgpu::TextureViewDescriptor::default()));

        let cache_bind = BindGroupBuilder::new().image(&cache_view, wgpu::ShaderStages::FRAGMENT);
        let cache_bind = ArcBindGroup::new(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &cache_bind_layout,
            entries: cache_bind.entries(),
        }));

        let verts = GrowingBufferArena::new(
            device,
            1,
            wgpu::BufferDescriptor {
                label: None,
                size: Self::MAX_TEXT_VERTEX_ARENA * std::mem::size_of::<TextVertex>() as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            },
        );

        TextRenderer {
            glyph_brush: RefCell::new(glyph_brush),
            cache,
            cache_view,
            cache_bind,
            cache_bind_layout,
            cache_size,

            verts,
        }
    }

    fn create_verts_buffer(device: &wgpu::Device, num_verts: usize) -> ArcBuffer {
        ArcBuffer::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (num_verts * std::mem::size_of::<TextVertex>()) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        }))
    }

    pub fn queue(&self, section: glyph_brush::Section<'_, Extra>) {
        self.glyph_brush.borrow_mut().queue(section);
    }

    #[allow(unsafe_code)]
    pub(crate) fn draw_queued<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        arenas: &'a FrameArenas,
        pass: &mut wgpu::RenderPass<'a>,
    ) {
        let res = self.glyph_brush.borrow_mut().process_queued(
            |rect, pixels| {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.cache,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: rect.min[0],
                            y: rect.min[1],
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(rect.width()),
                        rows_per_image: None,
                    },
                    wgpu::Extent3d {
                        width: rect.width(),
                        height: rect.height(),
                        depth_or_array_layers: 1,
                    },
                );
            },
            |glyph| TextVertex {
                rect: [
                    glyph.pixel_coords.min.x,
                    glyph.pixel_coords.min.y,
                    glyph.pixel_coords.max.x,
                    glyph.pixel_coords.max.y,
                ],
                uv: [
                    glyph.tex_coords.min.x,
                    glyph.tex_coords.min.y,
                    glyph.tex_coords.max.x,
                    glyph.tex_coords.max.y,
                ],
                color: glyph.extra.color.into(),
                transform_c0: glyph.extra.transform.to_cols_array_2d()[0],
                transform_c1: glyph.extra.transform.to_cols_array_2d()[1],
                transform_c2: glyph.extra.transform.to_cols_array_2d()[2],
                transform_c3: glyph.extra.transform.to_cols_array_2d()[3],
            },
        );

        match res {
            Ok(glyph_brush::BrushAction::Draw(verts)) => {
                let verts_size = verts.len() * std::mem::size_of::<TextVertex>();
                let (buffer, offset) = if verts.len() as u64 > Self::MAX_TEXT_VERTEX_ARENA {
                    (Self::create_verts_buffer(device, verts.len()), 0)
                } else {
                    let verts_alloc = self.verts.allocate(device, verts_size as u64);
                    (verts_alloc.buffer, verts_alloc.offset)
                };

                queue.write_buffer(&buffer, offset, bytemuck::cast_slice(verts.as_slice()));

                let verts_buf = arenas.buffers.alloc(buffer);
                pass.set_vertex_buffer(0, verts_buf.slice(offset..));

                // N.B.: 1 glyph = 4 verts, then n glyphs = n instances.
                // Also note that vertex data is stepped PER INSTANCE.
                // Therefore we only store ONE VERTEX for ONE GLYPH (and in the vertex shader we generate the quad vertices on the fly).
                pass.draw(0..4, 0..verts.len() as u32);
            }
            Err(glyph_brush::BrushError::TextureTooSmall { suggested }) => {
                // increase texture size as recommended by glyph_brush

                self.cache_size = suggested;
                self.glyph_brush
                    .borrow_mut()
                    .resize_texture(self.cache_size.0, self.cache_size.1);

                self.cache = ArcTexture::new(device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: self.cache_size.0,
                        height: self.cache_size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                }));

                self.cache_view = ArcTextureView::new(
                    self.cache
                        .create_view(&wgpu::TextureViewDescriptor::default()),
                );

                let cache_bind =
                    BindGroupBuilder::new().image(&self.cache_view, wgpu::ShaderStages::FRAGMENT);
                self.cache_bind =
                    ArcBindGroup::new(device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &self.cache_bind_layout,
                        entries: cache_bind.entries(),
                    }));

                self.draw_queued(device, queue, arenas, pass)
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Extra {
    pub color: LinearColor,
    pub transform: glam::Mat4,
}

// hash is impl'd via OrderedFloat, but we still want to preserve the types
#[allow(clippy::derived_hash_with_manual_eq)]
impl std::hash::Hash for Extra {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        [
            OrderedFloat::from(self.color.r),
            OrderedFloat::from(self.color.g),
            OrderedFloat::from(self.color.b),
            OrderedFloat::from(self.color.a),
        ]
        .hash(state);

        self.transform
            .to_cols_array()
            .into_iter()
            .for_each(|x| OrderedFloat::from(x).hash(state));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub struct TextVertex {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
    pub transform_c0: [f32; 4],
    pub transform_c1: [f32; 4],
    pub transform_c2: [f32; 4],
    pub transform_c3: [f32; 4],
}

impl TextVertex {
    pub(crate) const fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 7] = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 64,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 80,
                shader_location: 5,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 96,
                shader_location: 6,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        }
    }
}
