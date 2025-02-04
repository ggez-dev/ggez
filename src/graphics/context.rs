use super::{
    gpu::{
        bind_group::{BindGroupCache, BindGroupEntryKey},
        growing::GrowingBufferArena,
        pipeline::PipelineCache,
        text::TextRenderer,
    },
    image::{Image, ImageFormat},
    mesh::{Mesh, Vertex},
    sampler::{Sampler, SamplerCache},
    text::FontData,
    MeshData, ScreenImage,
};
use crate::{
    conf::{self, Backend, Conf, FullscreenType, WindowMode},
    context::Has,
    error::GameResult,
    filesystem::{Filesystem, InternalClone},
    graphics::gpu::{bind_group::BindGroupLayoutBuilder, pipeline::RenderPipelineInfo},
    GameError,
};
use glyph_brush::FontId;
use image as imgcrate;
use std::{collections::HashMap, path::Path, sync::Arc};
use winit::dpi::{self, PhysicalPosition};

pub(crate) struct FrameContext {
    pub cmd: wgpu::CommandEncoder,
    pub present: Image,
    pub frame: wgpu::SurfaceTexture,
    pub frame_view: wgpu::TextureView,
}

/// WGPU graphics context objects.
#[derive(Debug)]
#[allow(missing_docs)]
pub struct WgpuContext {
    pub instance: wgpu::Instance,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// A concrete graphics context for WGPU rendering.
#[allow(missing_debug_implementations)]
pub struct GraphicsContext {
    pub(crate) wgpu: Arc<WgpuContext>,

    pub(crate) window: Arc<winit::window::Window>,
    pub(crate) surface_config: wgpu::SurfaceConfiguration,

    pub(crate) bind_group_cache: BindGroupCache,
    pub(crate) pipeline_cache: PipelineCache,
    pub(crate) sampler_cache: SamplerCache,

    pub(crate) window_mode: WindowMode,
    pub(crate) frame: Option<ScreenImage>,
    pub(crate) frame_msaa: Option<ScreenImage>,
    pub(crate) frame_image: Option<Image>,
    pub(crate) frame_msaa_image: Option<Image>,

    pub(crate) fcx: Option<FrameContext>,
    pub(crate) text: TextRenderer,
    pub(crate) fonts: HashMap<String, FontId>,
    pub(crate) uniform_arena: GrowingBufferArena,

    pub(crate) draw_shader: wgpu::ShaderModule,

    #[cfg(feature = "3d")]
    pub(crate) draw_shader_3d: wgpu::ShaderModule,
    #[cfg(feature = "3d")]
    pub(crate) instance_shader_3d: wgpu::ShaderModule,
    #[cfg(feature = "3d")]
    pub(crate) instance_unordered_shader_3d: wgpu::ShaderModule,

    pub(crate) instance_shader: wgpu::ShaderModule,
    pub(crate) instance_unordered_shader: wgpu::ShaderModule,
    pub(crate) text_shader: wgpu::ShaderModule,
    pub(crate) copy_shader: wgpu::ShaderModule,
    pub(crate) rect_mesh: Mesh,
    pub(crate) white_image: Image,
    pub(crate) instance_bind_layout: wgpu::BindGroupLayout,

    pub(crate) fs: Filesystem,

    bind_group: Option<([BindGroupEntryKey; 2], wgpu::BindGroup)>,
}

impl GraphicsContext {
    /// Create a new graphics context
    pub fn new(
        game_id: &str,
        event_loop: &winit::event_loop::EventLoop<()>,
        conf: &Conf,
        filesystem: &Filesystem,
    ) -> GameResult<Self> {
        let new_instance = |backends| {
            wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends,
                ..Default::default()
            })
        };

        if conf.backend == Backend::All {
            match Self::new_from_instance(
                game_id,
                new_instance(wgpu::Backends::PRIMARY),
                event_loop,
                conf,
                filesystem,
            ) {
                Ok(o) => Ok(o),
                Err(GameError::GraphicsInitializationError) => {
                    println!(
                        "Failed to initialize graphics, trying secondary backends.. Please mention this if you encounter any bugs!"
                    );
                    warn!(
                        "Failed to initialize graphics, trying secondary backends.. Please mention this if you encounter any bugs!"
                    );

                    Self::new_from_instance(
                        game_id,
                        new_instance(wgpu::Backends::SECONDARY),
                        event_loop,
                        conf,
                        filesystem,
                    )
                }
                Err(e) => Err(e),
            }
        } else {
            let instance = new_instance(match conf.backend {
                Backend::All => unreachable!(),
                Backend::OnlyPrimary => wgpu::Backends::PRIMARY,
                Backend::Vulkan => wgpu::Backends::VULKAN,
                Backend::Metal => wgpu::Backends::METAL,
                Backend::Dx12 => wgpu::Backends::DX12,
                Backend::Gl => wgpu::Backends::GL,
                Backend::BrowserWebGpu => wgpu::Backends::BROWSER_WEBGPU,
            });

            Self::new_from_instance(game_id, instance, event_loop, conf, filesystem)
        }
    }

    fn bind_group(
        &mut self,
        view: wgpu::TextureView,
        sampler: wgpu::Sampler,
    ) -> (wgpu::BindGroup, wgpu::BindGroupLayout) {
        let key = [
            BindGroupEntryKey::Image(view.clone()),
            BindGroupEntryKey::Sampler(sampler.clone()),
        ];
        let layout = BindGroupLayoutBuilder::new()
            .image(wgpu::ShaderStages::FRAGMENT)
            .sampler(wgpu::ShaderStages::FRAGMENT)
            .create(&self.wgpu.device, &mut self.bind_group_cache);

        let bind_group = match &self.bind_group {
            Some((old_key, bind_group)) if old_key == &key => bind_group.clone(),
            _ => {
                let bind_group = self
                    .wgpu
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&sampler),
                            },
                        ],
                    });
                self.bind_group = Some((key, bind_group.clone()));
                bind_group
            }
        };

        (bind_group, layout)
    }

    pub(crate) fn new_from_instance(
        #[allow(unused_variables)] game_id: &str,
        instance: wgpu::Instance,
        event_loop: &winit::event_loop::EventLoop<()>,
        conf: &Conf,
        filesystem: &Filesystem,
    ) -> GameResult<Self> {
        let mut window_builder = winit::window::Window::default_attributes()
            .with_title(conf.window_setup.title.clone())
            .with_inner_size(conf.window_mode.actual_size().unwrap()) // Unwrap since actual_size only fails if one of the window dimensions is less than 1
            .with_resizable(conf.window_mode.resizable)
            .with_visible(conf.window_mode.visible)
            .with_transparent(conf.window_mode.transparent);

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            {
                use winit::platform::x11::WindowAttributesExtX11;
                window_builder = window_builder.with_name(game_id, game_id);
            }
            {
                use winit::platform::wayland::WindowAttributesExtWayland;
                window_builder = window_builder.with_name(game_id, game_id);
            }
        }

        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            window_builder = window_builder
                .with_drag_and_drop(false)
                .with_clip_children(false);
        }

        window_builder = if !conf.window_setup.icon.is_empty() {
            let icon = load_icon(conf.window_setup.icon.as_ref(), filesystem)?;
            window_builder.with_window_icon(Some(icon))
        } else {
            window_builder
        };

        // TODO remove deprecated create_window usage
        // In order to do this, we need to switch window creation to a point inside the active event loop instead of before.
        #[allow(deprecated)]
        let window = Arc::new(event_loop.create_window(window_builder)?);
        let surface = instance
            .create_surface(window.clone())
            .map_err(|_| GameError::GraphicsInitializationError)?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .ok_or(GameError::GraphicsInitializationError)?;

        // One instance is 96 bytes, and we allow 1 million of them, for a total of 96MB (default being 128MB).
        const MAX_INSTANCES: u32 = 1_000_000;
        const INSTANCE_BUFFER_SIZE: u32 = 96 * MAX_INSTANCES;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::default(),
                required_limits: wgpu::Limits {
                    // 1st: DrawParams
                    // 2nd: Texture + Sampler
                    // 3rd: InstanceArray
                    // 4th: ShaderParams
                    max_bind_groups: 4,
                    // InstanceArray uses 2 storage buffers.
                    max_storage_buffers_per_shader_stage: 2,
                    max_storage_buffer_binding_size: INSTANCE_BUFFER_SIZE,
                    max_texture_dimension_1d: 8192,
                    max_texture_dimension_2d: 8192,
                    ..wgpu::Limits::downlevel_webgl2_defaults()
                },
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))?;

        let wgpu = Arc::new(WgpuContext {
            instance,
            surface,
            device,
            queue,
        });

        let capabilities = wgpu.surface.get_capabilities(&adapter);

        let size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: capabilities.formats[0],
            width: size.width,
            height: size.height,
            present_mode: if conf.window_setup.vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };

        wgpu.surface.configure(&wgpu.device, &surface_config);

        let mut bind_group_cache = BindGroupCache::new();
        let pipeline_cache = PipelineCache::new();
        let sampler_cache = SamplerCache::new();

        let image_bind_layout = BindGroupLayoutBuilder::new()
            .image(wgpu::ShaderStages::FRAGMENT)
            .create(&wgpu.device, &mut bind_group_cache);

        let text = TextRenderer::new(&wgpu.device, image_bind_layout);

        let uniform_arena = GrowingBufferArena::new(
            &wgpu.device,
            u64::from(wgpu.device.limits().min_uniform_buffer_offset_alignment),
            wgpu::BufferDescriptor {
                label: None,
                size: 4096 * wgpu.device.limits().min_uniform_buffer_offset_alignment as u64, // Set to min buffer alignment so we can upload all uniform data at once
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );

        let load_shader = |source: &str| {
            wgpu.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                })
        };

        let draw_shader = load_shader(include_str!("shader/draw.wgsl"));

        #[cfg(feature = "3d")]
        let draw_shader_3d = load_shader(include_str!("shader/draw3d.wgsl"));
        #[cfg(feature = "3d")]
        let instance_shader_3d = load_shader(include_str!("shader/instance3d.wgsl"));
        #[cfg(feature = "3d")]
        let instance_unordered_shader_3d =
            load_shader(include_str!("shader/instance_unordered3d.wgsl"));

        let instance_shader = load_shader(include_str!("shader/instance.wgsl"));
        let instance_unordered_shader = load_shader(include_str!("shader/instance_unordered.wgsl"));
        let text_shader = load_shader(include_str!("shader/text.wgsl"));
        let copy_shader = load_shader(include_str!("shader/copy.wgsl"));

        let rect_mesh = Mesh::from_data_wgpu(
            &wgpu,
            MeshData {
                vertices: &[
                    Vertex {
                        position: [0., 0.],
                        uv: [0., 0.],
                        color: [1.; 4],
                    },
                    Vertex {
                        position: [1., 0.],
                        uv: [1., 0.],
                        color: [1.; 4],
                    },
                    Vertex {
                        position: [0., 1.],
                        uv: [0., 1.],
                        color: [1.; 4],
                    },
                    Vertex {
                        position: [1., 1.],
                        uv: [1., 1.],
                        color: [1.; 4],
                    },
                ],
                indices: &[0, 2, 1, 2, 3, 1],
            },
        );

        let instance_bind_layout = BindGroupLayoutBuilder::new()
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
            .create(&wgpu.device, &mut bind_group_cache);

        let white_image =
            Image::from_pixels_wgpu(&wgpu, &[255, 255, 255, 255], ImageFormat::Rgba8Unorm, 1, 1);

        let mut this = GraphicsContext {
            wgpu,

            window,
            surface_config,

            bind_group_cache,
            pipeline_cache,
            sampler_cache,

            window_mode: conf.window_mode,
            frame: None,
            frame_msaa: None,
            frame_image: None,
            frame_msaa_image: None,

            fcx: None,
            text,
            fonts: HashMap::new(),
            uniform_arena,
            draw_shader,

            #[cfg(feature = "3d")]
            draw_shader_3d,
            #[cfg(feature = "3d")]
            instance_shader_3d,
            #[cfg(feature = "3d")]
            instance_unordered_shader_3d,

            instance_shader,
            instance_unordered_shader,
            text_shader,
            copy_shader,
            rect_mesh,
            white_image,
            instance_bind_layout,

            fs: InternalClone::clone(filesystem),

            bind_group: None,
        };

        this.set_window_mode(&conf.window_mode)?;

        this.frame = Some(ScreenImage::new(&this, 1., 1., 1));
        this.frame_msaa = Some(ScreenImage::new(
            &this,
            1.,
            1.,
            u8::from(conf.window_setup.samples).into(),
        ));
        this.update_frame_image();

        this.add_font(
            "LiberationMono-Regular",
            FontData::from_slice(include_bytes!("../../resources/LiberationMono-Regular.ttf"))?,
        );

        Ok(this)
    }

    /// Returns a reference to the underlying WGPU context.
    #[inline]
    pub fn wgpu(&self) -> &WgpuContext {
        &self.wgpu
    }

    /// Sets the image that will be presented to the screen at the end of the frame.
    pub fn present(&mut self, image: &Image) -> GameResult {
        if let Some(fcx) = &mut self.fcx {
            fcx.present = image.clone();
            Ok(())
        } else {
            Err(GameError::RenderError(String::from(
                "cannot present outside of a frame",
            )))
        }
    }

    /// Adds a new `font` with a given `name`.
    #[allow(unused_results)]
    pub fn add_font(&mut self, name: &str, font: FontData) {
        let id = self.text.glyph_brush.borrow_mut().add_font(font.font);
        self.fonts.insert(name.to_string(), id);
    }

    /// Checks if the given font is loaded
    pub fn has_font(&self, font_name: impl Into<String>) -> bool {
        self.fonts.contains_key(&font_name.into())
    }

    /// Returns the size of the window’s underlying drawable in physical pixels as (width, height).
    pub fn drawable_size(&self) -> (f32, f32) {
        let size = self.window.inner_size();
        (size.width as f32, size.height as f32)
    }

    /// Sets the window size (in physical pixels) / resolution to the specified width and height.
    ///
    /// Note:   These dimensions are only interpreted as resolutions in true fullscreen mode.
    ///         If the selected resolution is not supported this function will return an Error.
    pub fn set_drawable_size(&mut self, width: f32, height: f32) -> GameResult {
        self.set_mode(self.window_mode.dimensions(width, height))
    }

    /// Sets the window title.
    pub fn set_window_title(&self, title: &str) {
        self.window.set_title(title);
    }

    /// Returns the position of the system window, including the outer frame.
    pub fn window_position(&self) -> GameResult<PhysicalPosition<i32>> {
        self.window
            .outer_position()
            .map_err(|e| GameError::WindowError(e.to_string()))
    }

    /// Sets the window position.
    pub fn set_window_position(&self, position: impl Into<winit::dpi::Position>) -> GameResult {
        self.window.set_outer_position(position);
        Ok(())
    }

    /// Returns the size of the window in pixels as (width, height),
    /// including borders, titlebar, etc.
    /// Returns zeros if the window doesn't exist.
    pub fn size(&self) -> (f32, f32) {
        let size = self.window.outer_size();
        (size.width as f32, size.height as f32)
    }

    /// Returns an iterator providing all resolutions supported by the current monitor.
    pub fn supported_resolutions(&self) -> impl Iterator<Item = winit::dpi::PhysicalSize<u32>> {
        self.window
            .current_monitor()
            .into_iter()
            .flat_map(|monitor| monitor.video_modes().map(|vm| vm.size()))
    }

    /// Returns a reference to the Winit window.
    #[inline]
    pub fn window(&self) -> &winit::window::Window {
        &self.window
    }

    /// Sets the window icon. `None` for path removes the icon.
    pub fn set_window_icon<P: AsRef<Path>>(
        &self,
        filesystem: &impl Has<Filesystem>,
        path: impl Into<Option<P>>,
    ) -> GameResult {
        let filesystem = filesystem.retrieve();
        let icon = match path.into() {
            Some(p) => Some(load_icon(p.as_ref(), filesystem)?),
            None => None,
        };
        self.window.set_window_icon(icon);
        Ok(())
    }

    /// Sets the window to fullscreen or back.
    pub fn set_fullscreen(&mut self, fullscreen: conf::FullscreenType) -> GameResult {
        let window_mode = self.window_mode.fullscreen_type(fullscreen);
        self.set_mode(window_mode)
    }

    /// Sets whether or not the window is resizable.
    pub fn set_resizable(&mut self, resizable: bool) -> GameResult {
        let window_mode = self.window_mode.resizable(resizable);
        self.set_mode(window_mode)
    }

    /// Sets the window mode, such as the size and other properties.
    ///
    /// Setting the window mode may have side effects, such as clearing
    /// the screen or setting the screen coordinates viewport to some
    /// undefined value (for example, the window was resized).  It is
    /// recommended to call
    /// [`set_screen_coordinates()`](fn.set_screen_coordinates.html) after
    /// changing the window size to make sure everything is what you want
    /// it to be.
    pub fn set_mode(&mut self, mut mode: WindowMode) -> GameResult {
        let old_fullscreen = self.window_mode.fullscreen_type;
        let result = self.set_window_mode(&mode);
        if let Err(GameError::WindowError(_)) = result {
            mode.fullscreen_type = old_fullscreen;
        }
        self.window_mode = mode;
        result
    }

    /// Returns the default frame image.
    ///
    /// This is the image that is rendered to when `Canvas::from_frame` is used.
    #[inline]
    pub fn frame(&self) -> &Image {
        self.frame_image.as_ref().unwrap(/* invariant */)
    }

    /// Returns the image format of the window surface.
    #[inline]
    pub fn surface_format(&self) -> ImageFormat {
        self.surface_config.format
    }

    /// Returns the current [`wgpu::CommandEncoder`] if there is a frame in progress.
    pub fn commands(&mut self) -> Option<&mut wgpu::CommandEncoder> {
        self.fcx.as_mut().map(|fcx| &mut fcx.cmd)
    }

    /// Begins a new frame.
    ///
    /// The only situation you need to call this in is when you are rolling your own event loop.
    pub fn begin_frame(&mut self) -> GameResult {
        if self.fcx.is_some() {
            return Err(GameError::RenderError(String::from(
                "cannot begin a new frame while another frame is still in progress; call end_frame first",
            )));
        }

        let size = self.window.inner_size();
        let frame = match self.wgpu.surface.get_current_texture() {
            Ok(frame) => Ok(frame),
            Err(_) => {
                self.surface_config.width = size.width.max(1);
                self.surface_config.height = size.height.max(1);
                self.wgpu
                    .surface
                    .configure(&self.wgpu.device, &self.surface_config);
                self.wgpu.surface.get_current_texture().map_err(|_| {
                    GameError::RenderError(String::from("failed to get next swapchain image"))
                })
            }
        }?;

        let frame_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.fcx = Some(FrameContext {
            cmd: self
                .wgpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default()),
            present: self.frame().clone(),
            frame,
            frame_view,
        });

        self.uniform_arena.free();

        self.text.verts.free();

        Ok(())
    }

    /// Ends the current frame.
    ///
    /// The only situation you need to call this in is when you are rolling your own event loop.
    pub fn end_frame(&mut self) -> GameResult {
        if let Some(mut fcx) = self.fcx.take() {
            let mut present_pass = fcx.cmd.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &fcx.frame_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            let sampler = self
                .sampler_cache
                .get(&self.wgpu.device, Sampler::default());

            let (bind, layout) = self.bind_group(fcx.present.view, sampler);

            let layout = self.pipeline_cache.layout(&self.wgpu.device, &[&layout]);
            let copy = self.pipeline_cache.render_pipeline(
                &self.wgpu.device,
                RenderPipelineInfo {
                    layout,
                    vs: self.copy_shader.clone(),
                    fs: self.copy_shader.clone(),
                    vs_entry: "vs_main".into(),
                    fs_entry: "fs_main".into(),
                    samples: 1,
                    format: self.surface_config.format,
                    blend: None,
                    depth: None,
                    vertices: false,
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    vertex_layout: Vertex::layout(),
                    cull_mode: None,
                },
            );

            present_pass.set_pipeline(&copy);
            present_pass.set_bind_group(0, &bind, &[]);
            present_pass.draw(0..3, 0..1);

            std::mem::drop(present_pass);

            let _ = self.wgpu.queue.submit([fcx.cmd.finish()]);
            fcx.frame.present();

            Ok(())
        } else {
            Err(GameError::RenderError(String::from(
                "cannot end a frame as there was never one in progress; call begin_frame first",
            )))
        }
    }

    pub(crate) fn resize(&mut self, _new_size: dpi::PhysicalSize<u32>) {
        let size = self.window.inner_size();
        let _ = self.wgpu.device.poll(wgpu::Maintain::Wait);
        self.surface_config.width = size.width.max(1);
        self.surface_config.height = size.height.max(1);
        self.wgpu
            .surface
            .configure(&self.wgpu.device, &self.surface_config);
        self.update_frame_image();
    }

    pub(crate) fn update_frame_image(&mut self) {
        // Internally, GraphicsContext stores an intermediate image that is rendered to. Then, that frame image is rendered to the actual swapchain image.
        // Moreover, one frame image is non-MSAA, whilst the other is MSAA.
        // Since they're stored as ScreenImage, all this function does is store the corresponding Image returned by `ScreenImage::image()`.

        let mut frame = self.frame.take().unwrap(/* invariant */);
        self.frame_image = Some(frame.image(self));
        self.frame = Some(frame);

        let mut frame_msaa = self.frame_msaa.take().unwrap(/* invariant */);
        self.frame_msaa_image = Some(frame_msaa.image(self));
        self.frame_msaa = Some(frame_msaa);
    }

    pub(crate) fn set_window_mode(&mut self, mode: &WindowMode) -> GameResult {
        let window = &mut self.window;

        // TODO LATER: find out if single-dimension constraints are possible?
        let min_dimensions = if mode.min_width >= 1.0 && mode.min_height >= 1.0 {
            Some(dpi::PhysicalSize {
                width: f64::from(mode.min_width),
                height: f64::from(mode.min_height),
            })
        } else {
            return Err(GameError::WindowError(format!(
                "window min_width and min_height need to be at least 1; actual values: {}, {}",
                mode.min_width, mode.min_height
            )));
        };
        window.set_min_inner_size(min_dimensions);

        let max_dimensions = if mode.max_width > 0.0 && mode.max_height > 0.0 {
            Some(dpi::PhysicalSize {
                width: f64::from(mode.max_width),
                height: f64::from(mode.max_height),
            })
        } else {
            None
        };
        window.set_max_inner_size(max_dimensions);
        window.set_visible(mode.visible);

        match mode.fullscreen_type {
            FullscreenType::Windowed => {
                window.set_fullscreen(None);
                window.set_decorations(!mode.borderless);
                let _ = window.request_inner_size(mode.actual_size()?);
                window.set_resizable(mode.resizable);
                window.set_maximized(mode.maximized);
            }
            FullscreenType::True => {
                if let Some(monitor) = window.current_monitor() {
                    let v_modes = monitor.video_modes();
                    // try to find a video mode with a matching resolution
                    let mut match_found = false;
                    for v_mode in v_modes {
                        let size = v_mode.size();
                        if (size.width, size.height) == (mode.width as u32, mode.height as u32) {
                            window
                                .set_fullscreen(Some(winit::window::Fullscreen::Exclusive(v_mode)));
                            match_found = true;
                            break;
                        }
                    }
                    if !match_found {
                        return Err(GameError::WindowError(format!(
                            "resolution {}x{} is not supported by this monitor",
                            mode.width, mode.height
                        )));
                    }
                }
            }
            FullscreenType::Desktop => {
                window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            }
        }

        let size = window.inner_size();
        assert!(size.width > 0 && size.height > 0);
        self.surface_config.width = size.width.max(1);
        self.surface_config.height = size.height.max(1);

        self.wgpu
            .surface
            .configure(&self.wgpu.device, &self.surface_config);

        Ok(())
    }
}

// This is kinda awful 'cause it copies a couple times,
// but still better than
// having `winit` try to do the image loading for us.
// see https://github.com/tomaka/winit/issues/661
pub(crate) fn load_icon(
    icon_file: &Path,
    filesystem: &Filesystem,
) -> GameResult<winit::window::Icon> {
    use std::io::Read;
    use winit::window::Icon;

    let mut buf = Vec::new();
    let mut reader = filesystem.open(icon_file)?;
    let _ = reader.read_to_end(&mut buf)?;
    let i = imgcrate::load_from_memory(&buf)?;
    let image_data = i.to_rgba8();
    Icon::from_rgba(image_data.to_vec(), i.width(), i.height()).map_err(|e| {
        let msg = format!("Could not load icon: {e:?}");
        GameError::ResourceLoadError(msg)
    })
}
