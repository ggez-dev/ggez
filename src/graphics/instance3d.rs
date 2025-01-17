use crate::{context::Has, graphics::gpu::bind_group::BindGroupBuilder, GameError, GameResult};

use super::{
    context::GraphicsContext,
    gpu::arc::{ArcBindGroup, ArcBindGroupLayout, ArcBuffer},
    internal_canvas3d::InstanceArrayView3d,
    Canvas3d, Draw3d, DrawParam3d, DrawUniforms3d, Drawable3d, Image, Mesh3d, Std140DrawUniforms3d,
    WgpuContext,
};
use crevice::std140::AsStd140;
use std::{
    cmp::Ordering,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
        Mutex,
    },
};

const DEFAULT_CAPACITY: usize = 16;

/// Array of instances for fast rendering of many meshes.
#[derive(Debug)]
pub struct InstanceArray3d {
    pub(crate) buffer: Mutex<ArcBuffer>,
    pub(crate) indices: Mutex<ArcBuffer>,
    pub(crate) bind_group: Mutex<ArcBindGroup>,
    pub(crate) bind_layout: ArcBindGroupLayout,
    pub(crate) image: Image,
    pub(crate) mesh: Mesh3d,
    pub(crate) ordered: bool,
    dirty: AtomicBool,
    capacity: AtomicUsize,
    uniforms: Vec<Std140DrawUniforms3d>,
    params: Vec<DrawParam3d>,
    sort_by: fn(&DrawParam3d, &DrawParam3d) -> Ordering,
}

impl InstanceArray3d {
    /// Creates a new [`InstanceArray3d`] capable of storing up to n-`capacity` instances
    /// (this can be changed and is resized automatically when needed).
    ///
    /// If `image` is `None`, a 1x1 white image will be used to texture meshes.
    ///
    /// This constructor is `unordered` meaning instances will be drawn by their push/index order. Use [`InstanceArray::new_ordered`] to order by z-value.
    pub fn new(
        gfx: &impl Has<GraphicsContext>,
        image: impl Into<Option<Image>>,
        mesh: Mesh3d,
    ) -> Self {
        let gfx = gfx.retrieve();
        InstanceArray3d::new_wgpu(
            &gfx.wgpu,
            gfx.instance_bind_layout.clone(),
            image.into().unwrap_or_else(|| gfx.white_image.clone()),
            DEFAULT_CAPACITY,
            false,
            mesh,
        )
    }

    /// See [`InstanceArray3d::new`] for details.
    ///
    /// This constructor is `ordered` meaning instances will be drawn by their z-value at a slight performance cost. Use [`InstanceArray3d::new`] to order by index.
    pub fn new_ordered(
        gfx: &impl Has<GraphicsContext>,
        image: impl Into<Option<Image>>,
        mesh: Mesh3d,
    ) -> Self {
        let gfx = gfx.retrieve();
        InstanceArray3d::new_wgpu(
            &gfx.wgpu,
            gfx.instance_bind_layout.clone(),
            image.into().unwrap_or_else(|| gfx.white_image.clone()),
            DEFAULT_CAPACITY,
            true,
            mesh,
        )
    }

    fn new_wgpu(
        wgpu: &WgpuContext,
        bind_layout: ArcBindGroupLayout,
        image: Image,
        capacity: usize,
        ordered: bool,
        mesh: Mesh3d,
    ) -> Self {
        assert!(capacity > 0);

        let buffer = wgpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: DrawUniforms3d::std140_size_static() as u64 * capacity as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let indices = wgpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: if ordered {
                std::mem::size_of::<u32>() as u64 * capacity as u64
            } else {
                4 // min for layout
            },
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = BindGroupBuilder::new()
            .buffer(
                &buffer,
                0,
                wgpu::ShaderStages::VERTEX,
                wgpu::BufferBindingType::Storage { read_only: true },
                false,
                None,
            )
            .buffer(
                &indices,
                0,
                wgpu::ShaderStages::VERTEX,
                wgpu::BufferBindingType::Storage { read_only: true },
                false,
                None,
            );
        let bind_group = wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_layout,
            entries: bind_group.entries(),
        });

        let uniforms = Vec::with_capacity(capacity);
        let params = Vec::with_capacity(capacity);

        InstanceArray3d {
            buffer: Mutex::new(buffer),
            indices: Mutex::new(indices),
            bind_group: Mutex::new(bind_group),
            bind_layout,
            image,
            ordered,
            dirty: AtomicBool::new(false),
            capacity: AtomicUsize::new(capacity),
            uniforms,
            params,
            mesh,
            sort_by: |a, b| a.z.cmp(&b.z),
        }
    }

    /// Resets all the instance data to a set of `DrawParam3d`.
    pub fn set(&mut self, instances: impl IntoIterator<Item = DrawParam3d>) {
        self.dirty.store(true, SeqCst);
        self.params.clear();
        self.params.extend(instances);
        self.uniforms.clear();
        self.uniforms.extend(
            self.params
                .iter()
                .map(|x| DrawUniforms3d::from_param(x).as_std140()),
        );
    }

    /// Pushes a new instance onto the end.
    pub fn push(&mut self, instance: DrawParam3d) {
        self.dirty.store(true, SeqCst);
        self.uniforms
            .push(DrawUniforms3d::from_param(&instance).as_std140());
        self.params.push(instance);
    }

    /// Updates an existing instance at a given index, if it is valid.
    pub fn update(&mut self, index: u32, instance: DrawParam3d) {
        if let Some((uniform, param)) = self
            .uniforms
            .get_mut(index as usize)
            .and_then(|x| Some((x, self.params.get_mut(index as usize)?)))
        {
            self.dirty.store(true, SeqCst);
            *uniform = DrawUniforms3d::from_param(&instance).as_std140();
            *param = instance;
        }
    }

    /// Clears all instance data.
    pub fn clear(&mut self) {
        // don't need to set dirty here
        self.uniforms.clear();
        self.params.clear();
    }

    /// Returns whether the instance data has been changed without being flushed (i.e., uploaded to the GPU).
    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(SeqCst)
    }

    /// Returns an immutable slice of all the instance data in this [`InstanceArray3d`].
    #[inline]
    pub fn instances(&self) -> &[DrawParam3d] {
        &self.params
    }

    pub(crate) fn flush_wgpu(&self, wgpu: &WgpuContext) -> GameResult {
        if !self.dirty.load(SeqCst) {
            return Ok(());
        } else {
            self.dirty.store(false, SeqCst);
        }

        let len = self.uniforms.len();
        //if len > self.capacity.load(SeqCst) {
        let mut resized = InstanceArray3d::new_wgpu(
            wgpu,
            self.bind_layout.clone(),
            self.image.clone(),
            len,
            self.ordered,
            self.mesh.clone(),
        );
        *self.buffer.lock().map_err(|_| GameError::LockError)? =
            resized.buffer.get_mut().unwrap().clone();
        *self.indices.lock().map_err(|_| GameError::LockError)? =
            resized.indices.get_mut().unwrap().clone();
        *self.bind_group.lock().map_err(|_| GameError::LockError)? =
            resized.bind_group.get_mut().unwrap().clone();
        self.capacity.store(len, SeqCst);
        //}

        wgpu.queue.write_buffer(
            &self.buffer.lock().unwrap(),
            0,
            bytemuck::cast_slice(self.uniforms.as_slice()),
        );

        if self.ordered {
            let mut sorted: Vec<_> = self.params.iter().enumerate().collect();
            sorted.sort_by(|(_, a), (_, b)| (self.sort_by)(a, b));
            let indices: Vec<_> = sorted.iter().map(|(i, _)| *i as u32).collect();
            wgpu.queue.write_buffer(
                &self.indices.lock().unwrap(),
                0,
                bytemuck::cast_slice(indices.as_slice()),
            );
        }

        Ok(())
    }

    /// Changes the capacity of this `InstanceArray3d` while preserving instances.
    ///
    /// If `new_capacity` is less than the `len`, the instances will be truncated.
    ///
    /// # Panics
    /// Panics if `new_capacity` is 0.
    pub fn resize(&mut self, gfx: &impl Has<GraphicsContext>, new_capacity: usize) {
        assert!(new_capacity > 0);

        let gfx: &GraphicsContext = gfx.retrieve();
        let resized = InstanceArray3d::new_wgpu(
            &gfx.wgpu,
            self.bind_layout.clone(),
            self.image.clone(),
            new_capacity,
            self.ordered,
            self.mesh.clone(),
        );
        self.buffer = resized.buffer;
        self.indices = resized.indices;
        self.bind_group = resized.bind_group;

        self.capacity.store(new_capacity, SeqCst);
        self.dirty.store(true, SeqCst);
        self.uniforms.truncate(new_capacity);
        self.params.truncate(new_capacity);
        self.uniforms.reserve(new_capacity - self.uniforms.len());
        self.params.reserve(new_capacity - self.params.len());
    }

    /// Returns this `InstanceArray3d`'s associated `image`.
    #[inline]
    pub fn image(&self) -> Image {
        self.image.clone()
    }

    /// Returns this `InstanceArray3d`'s associated `mesh`.
    #[inline]
    pub fn mesh(&self) -> Mesh3d {
        self.mesh.clone()
    }

    /// Returns the number of instances this [`InstanceArray3d`] is capable of holding.
    /// This number was specified when creating the [`InstanceArray3d`], or if the [`InstanceArray3d`]
    /// was automatically resized, the greatest length of instances.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity.load(SeqCst)
    }

    /// Sets the sorting function for the final draw order of the instance array.
    /// This is used upon the instance array being drawn to a canvas.
    ///
    /// By default, draws will be sorted by the Z index of [`DrawParam3d`].
    #[inline]
    pub fn set_sort_by(&mut self, sort_by: fn(&DrawParam3d, &DrawParam3d) -> Ordering) {
        self.sort_by = sort_by;
    }
}

impl Drawable3d for InstanceArray3d {
    fn draw(&self, canvas: &mut Canvas3d, param: impl Into<DrawParam3d>) {
        // Only flush (and then push a draw) if there are any instances to draw.
        // This guards against attempts to create empty buffers in `new_wgpu`, see #1168.
        if self.instances().is_empty() {
            return;
        }
        self.flush_wgpu(&canvas.wgpu).unwrap();
        canvas.push_draw(
            Draw3d::MeshInstances {
                mesh: (&self.mesh).into(),
                instances: InstanceArrayView3d::from_instances(self).unwrap(),
            },
            param.into(),
        );
    }
}
