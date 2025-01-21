use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    num::NonZeroU64,
};

/// Builder pattern for bind group layouts; basically just produces a Vec<BindGroupLayoutEntry>.
pub struct BindGroupLayoutBuilder {
    entries: Vec<wgpu::BindGroupLayoutEntry>,
    seed: u64,
}

impl BindGroupLayoutBuilder {
    pub fn new() -> Self {
        BindGroupLayoutBuilder {
            entries: vec![],
            seed: 0,
        }
    }

    pub fn seed(mut self, seed: impl Hash) -> Self {
        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        self.seed = hasher.finish();
        self
    }

    pub fn buffer(
        mut self,
        visibility: wgpu::ShaderStages,
        ty: wgpu::BufferBindingType,
        has_dynamic_offset: bool,
    ) -> Self {
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding: self.entries.len() as _,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset,
                min_binding_size: None,
            },
            count: None,
        });
        self
    }

    pub fn image(mut self, visibility: wgpu::ShaderStages) -> Self {
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding: self.entries.len() as _,
            visibility,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        self
    }

    pub fn sampler(mut self, visibility: wgpu::ShaderStages) -> Self {
        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding: self.entries.len() as _,
            visibility,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        self
    }

    pub fn create(
        self,
        device: &wgpu::Device,
        cache: &mut BindGroupCache,
    ) -> wgpu::BindGroupLayout {
        cache
            .layouts
            .entry((self.entries, self.seed))
            .or_insert_with_key(|(entries, _)| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries,
                })
            })
            .clone()
    }
}

/// This is used as a key into the HashMap cache to uniquely identify a bind group.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum BindGroupEntryKey {
    Buffer {
        buffer: wgpu::Buffer,
        offset: u64,
        size: Option<u64>,
    },
    Image(wgpu::TextureView),
    Sampler(wgpu::Sampler),
}

pub struct BindGroupBuilder<'a> {
    layout: BindGroupLayoutBuilder,
    entries: Vec<wgpu::BindGroupEntry<'a>>,
    key: Vec<BindGroupEntryKey>,
}

impl<'a> BindGroupBuilder<'a> {
    pub fn new() -> Self {
        BindGroupBuilder {
            layout: BindGroupLayoutBuilder::new(),
            entries: vec![],
            key: vec![],
        }
    }

    pub fn buffer(
        mut self,
        buffer: &'a wgpu::Buffer,
        offset: u64,
        visibility: wgpu::ShaderStages,
        ty: wgpu::BufferBindingType,
        has_dynamic_offset: bool,
        size: Option<u64>,
    ) -> Self {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.entries.len() as _,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset,
                size: size.map(|x| NonZeroU64::new(x).unwrap()), // Unwrap should always be nonzero
            }),
        });

        self.key.push(BindGroupEntryKey::Buffer {
            buffer: buffer.clone(),
            offset,
            size,
        });

        BindGroupBuilder {
            layout: self.layout.buffer(visibility, ty, has_dynamic_offset),
            entries: self.entries,
            key: self.key,
        }
    }

    pub fn image(mut self, view: &'a wgpu::TextureView, visibility: wgpu::ShaderStages) -> Self {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.entries.len() as _,
            resource: wgpu::BindingResource::TextureView(view),
        });

        self.key.push(BindGroupEntryKey::Image(view.clone()));

        BindGroupBuilder {
            layout: self.layout.image(visibility),
            entries: self.entries,
            key: self.key,
        }
    }

    pub fn sampler(mut self, sampler: &'a wgpu::Sampler, visibility: wgpu::ShaderStages) -> Self {
        self.entries.push(wgpu::BindGroupEntry {
            binding: self.entries.len() as _,
            resource: wgpu::BindingResource::Sampler(sampler),
        });

        self.key.push(BindGroupEntryKey::Sampler(sampler.clone()));

        BindGroupBuilder {
            layout: self.layout.sampler(visibility),
            entries: self.entries,
            key: self.key,
        }
    }

    pub fn create(
        self,
        device: &wgpu::Device,
        cache: &mut BindGroupCache,
    ) -> (wgpu::BindGroup, wgpu::BindGroupLayout) {
        let layout = self.layout.create(device, cache);

        let group = cache
            .groups
            .entry(self.key)
            .or_insert_with(|| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &layout,
                    entries: &self.entries,
                })
            })
            .clone();

        (group, layout)
    }

    #[allow(unused)]
    pub fn create_uncached(
        self,
        device: &wgpu::Device,
    ) -> (wgpu::BindGroup, wgpu::BindGroupLayout) {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &self.layout.entries,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &self.entries,
        });
        (group, layout)
    }

    #[inline]
    pub fn entries(&self) -> &[wgpu::BindGroupEntry] {
        &self.entries
    }
}

#[derive(Debug)]
pub struct BindGroupCache {
    layouts: HashMap<(Vec<wgpu::BindGroupLayoutEntry>, u64), wgpu::BindGroupLayout>,
    groups: HashMap<Vec<BindGroupEntryKey>, wgpu::BindGroup>,
}

impl BindGroupCache {
    pub fn new() -> Self {
        BindGroupCache {
            layouts: HashMap::new(),
            groups: HashMap::new(),
        }
    }
}
