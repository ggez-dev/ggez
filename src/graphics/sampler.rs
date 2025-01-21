use std::collections::HashMap;

/// Sampler state that is used when sampling images on the GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sampler {
    /// Clamping mode in the U (x) direction.
    pub clamp_u: ClampMode,
    /// Clamping mode in the V (y) direction.
    pub clamp_v: ClampMode,
    /// Clamping mode in the W (z) direction.
    pub clamp_w: ClampMode,
    /// Magnification (upscaling) filter.
    pub mag: FilterMode,
    /// Minification (downscaling) filter.
    pub min: FilterMode,
}

impl Sampler {
    /// Sampler state with linear filtering and edge clamping.
    pub fn linear_clamp() -> Self {
        Sampler {
            clamp_u: ClampMode::Clamp,
            clamp_v: ClampMode::Clamp,
            clamp_w: ClampMode::Clamp,
            mag: FilterMode::Linear,
            min: FilterMode::Linear,
        }
    }

    /// Sampler state with nearest filtering and edge clamping.
    ///
    /// Ideal for pixel art.
    pub fn nearest_clamp() -> Self {
        Sampler {
            mag: FilterMode::Nearest,
            min: FilterMode::Nearest,
            ..Self::linear_clamp()
        }
    }
}

impl Default for Sampler {
    fn default() -> Self {
        Self::linear_clamp()
    }
}

impl From<Sampler> for wgpu::SamplerDescriptor<'_> {
    fn from(sampler: Sampler) -> Self {
        wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: sampler.clamp_u.into(),
            address_mode_v: sampler.clamp_v.into(),
            address_mode_w: sampler.clamp_w.into(),
            mag_filter: sampler.mag.into(),
            min_filter: sampler.min.into(),
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }
}

impl From<FilterMode> for Sampler {
    fn from(filter: FilterMode) -> Self {
        match filter {
            FilterMode::Nearest => Self::nearest_clamp(),
            FilterMode::Linear => Self::linear_clamp(),
        }
    }
}

/// Describes the clamping mode of a sampler, used when the shader writes to sample outside of texture boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClampMode {
    /// The corresponding texel at the nearest edge is sampled.
    Clamp,
    /// The sample coordinates wrap, effectively repeating the texture.
    Repeat,
    /// The sample coordinates wrap and mirror, effectively repeating the texture and flipping.
    MirrorRepeat,
}

impl From<ClampMode> for wgpu::AddressMode {
    fn from(clamp: ClampMode) -> Self {
        match clamp {
            ClampMode::Clamp => wgpu::AddressMode::ClampToEdge,
            ClampMode::Repeat => wgpu::AddressMode::Repeat,
            ClampMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
        }
    }
}

/// Describes the filter mode of a sampler, used when magnification or minification of a texture occurs (i.e. scaling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FilterMode {
    /// The nearest texel is sampled.
    Nearest,
    /// The neighbouring texels are linearly interpolated.
    Linear,
}

impl From<FilterMode> for wgpu::FilterMode {
    fn from(filter: FilterMode) -> Self {
        match filter {
            FilterMode::Nearest => wgpu::FilterMode::Nearest,
            FilterMode::Linear => wgpu::FilterMode::Linear,
        }
    }
}

#[derive(Debug)]
pub(crate) struct SamplerCache {
    cache: HashMap<Sampler, wgpu::Sampler>,
}

impl SamplerCache {
    pub fn new() -> Self {
        SamplerCache {
            cache: Default::default(),
        }
    }

    pub fn get(&mut self, device: &wgpu::Device, sampler: Sampler) -> wgpu::Sampler {
        self.cache
            .entry(sampler)
            .or_insert_with(|| device.create_sampler(&sampler.into()))
            .clone()
    }
}
