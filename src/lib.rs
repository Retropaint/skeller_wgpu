use image::{DynamicImage, ImageResult, RgbaImage};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use wgpu::{BindGroup, BindGroupLayout, InstanceDescriptor};

use std::{fs, sync::Arc};
use web_time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    window::{Theme, Window},
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 2],
}

const VERTICES: [Vertex; 4] = [
    Vertex {
        position: [0.5, 0.5],
        color: [1., 0.],
    },
    Vertex {
        position: [-0.5, -0.5],
        color: [0., 1.],
    },
    Vertex {
        position: [-0.5, 0.5],
        color: [0., 0.],
    },
    Vertex {
        position: [0.5, -0.5],
        color: [1., 1.],
    },
];

const INDICES: [u32; 6] = [0, 1, 2, 0, 1, 3];

#[derive(Default)]
pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    gui_state: Option<egui_winit::State>,
    last_render_time: Option<Instant>,
    #[cfg(target_arch = "wasm32")]
    renderer_receiver: Option<futures::channel::oneshot::Receiver<Renderer>>,
    last_size: (u32, u32),
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let mut attributes = Window::default_attributes();

        #[cfg(not(target_arch = "wasm32"))]
        {
            attributes = attributes.with_title("Standalone Winit/Wgpu Example");
        }

        #[allow(unused_assignments)]
        #[cfg(target_arch = "wasm32")]
        let (mut canvas_width, mut canvas_height) = (0, 0);

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = wgpu::web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id("canvas")
                .unwrap()
                .dyn_into::<wgpu::web_sys::HtmlCanvasElement>()
                .unwrap();
            canvas_width = canvas.width();
            canvas_height = canvas.height();
            self.last_size = (canvas_width, canvas_height);
            attributes = attributes.with_canvas(Some(canvas));
        }

        if let Ok(window) = event_loop.create_window(attributes) {
            let first_window_handle = self.window.is_none();
            let window_handle = Arc::new(window);
            self.window = Some(window_handle.clone());
            if first_window_handle {
                let gui_context = egui::Context::default();

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let inner_size = window_handle.inner_size();
                    self.last_size = (inner_size.width, inner_size.height);
                }

                #[cfg(target_arch = "wasm32")]
                {
                    gui_context.set_pixels_per_point(window_handle.scale_factor() as f32);
                }

                let viewport_id = gui_context.viewport_id();
                let gui_state = egui_winit::State::new(
                    gui_context,
                    viewport_id,
                    &window_handle,
                    Some(window_handle.scale_factor() as _),
                    Some(Theme::Dark),
                    None,
                );

                #[cfg(not(target_arch = "wasm32"))]
                let (width, height) = (
                    window_handle.inner_size().width,
                    window_handle.inner_size().height,
                );

                #[cfg(not(target_arch = "wasm32"))]
                {
                    env_logger::init();
                    let renderer = pollster::block_on(async move {
                        Renderer::new(window_handle.clone(), width, height).await
                    });
                    self.renderer = Some(renderer);
                }

                #[cfg(target_arch = "wasm32")]
                {
                    let (sender, receiver) = futures::channel::oneshot::channel();
                    self.renderer_receiver = Some(receiver);
                    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
                    console_log::init().expect("Failed to initialize logger!");
                    log::info!("Canvas dimensions: ({canvas_width} x {canvas_height})");
                    wasm_bindgen_futures::spawn_local(async move {
                        let renderer =
                            Renderer::new(window_handle.clone(), canvas_width, canvas_height).await;
                        if sender.send(renderer).is_err() {
                            log::error!("Failed to create and send renderer!");
                        }
                    });
                }

                self.gui_state = Some(gui_state);
                self.last_render_time = Some(Instant::now());
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        #[cfg(target_arch = "wasm32")]
        {
            let mut renderer_received = false;
            if let Some(receiver) = self.renderer_receiver.as_mut() {
                if let Ok(Some(renderer)) = receiver.try_recv() {
                    self.renderer = Some(renderer);
                    renderer_received = true;
                }
            }
            if renderer_received {
                self.renderer_receiver = None;
            }
        }

        let (Some(gui_state), Some(renderer), Some(window), Some(last_render_time)) = (
            self.gui_state.as_mut(),
            self.renderer.as_mut(),
            self.window.as_ref(),
            self.last_render_time.as_mut(),
        ) else {
            return;
        };

        // Receive gui window event
        if gui_state.on_window_event(window, &event).consumed {
            return;
        }

        // If the gui didn't consume the event, handle it
        match event {
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key: winit::keyboard::PhysicalKey::Code(key_code),
                        ..
                    },
                ..
            } => {
                // Exit by pressing the escape key
                if matches!(key_code, winit::keyboard::KeyCode::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(PhysicalSize { width, height }) => {
                log::info!("Resizing renderer surface to: ({width}, {height})");
                renderer.resize(width, height);
                self.last_size = (width, height);
            }
            WindowEvent::CloseRequested => {
                log::info!("Close requested. Exiting...");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                *last_render_time = now;

                let input = gui_state.take_egui_input(window);
                gui_state.egui_ctx().begin_pass(input);

                #[cfg(not(target_arch = "wasm32"))]
                let title = "Rust/Wgpu";

                #[cfg(feature = "webgpu")]
                let title = "Rust/Wgpu/Webgpu";

                #[cfg(feature = "webgl")]
                let title = "Rust/Wgpu/Webgl";

                egui::Window::new(title).show(gui_state.egui_ctx(), |ui| {
                    //ui.checkbox(&mut self.panels_visible, "Show Panels");
                });

                let egui_winit::egui::FullOutput {
                    textures_delta,
                    shapes,
                    pixels_per_point,
                    platform_output,
                    ..
                } = gui_state.egui_ctx().end_pass();

                gui_state.handle_platform_output(window, platform_output);

                let paint_jobs = gui_state.egui_ctx().tessellate(shapes, pixels_per_point);

                let screen_descriptor = {
                    let (width, height) = self.last_size;
                    egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [width, height],
                        pixels_per_point: window.scale_factor() as f32,
                    }
                };

                renderer.render_frame(screen_descriptor, paint_jobs, textures_delta);
            }
            _ => (),
        }

        window.request_redraw();
    }
}

pub struct Renderer {
    gpu: Gpu,
    depth_texture_view: wgpu::TextureView,
    egui_renderer: egui_wgpu::Renderer,
    scene: Scene,
    bind_group_layout: BindGroupLayout,
}

impl Renderer {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub async fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Self {
        let gpu = Gpu::new_async(window, width, height).await;
        let depth_texture_view = gpu.create_depth_texture(width, height);

        let egui_renderer = egui_wgpu::Renderer::new(
            &gpu.device,
            gpu.surface_config.format,
            Some(Self::DEPTH_FORMAT),
            1,
            false,
        );

        let bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                    label: Some("texture_bind_group_layout"),
                });

        let scene = Scene::new(&gpu.device, gpu.surface_format, &bind_group_layout);

        Self {
            gpu,
            depth_texture_view,
            egui_renderer,
            scene,
            bind_group_layout,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
        self.depth_texture_view = self.gpu.create_depth_texture(width, height);
    }

    fn create_texture(&self, img_path: &str, bind_group_layout: BindGroupLayout) -> BindGroup {
        let diffuse_image: ImageResult<DynamicImage>;
        let diffuse_rgba: RgbaImage;
        let dimensions: (u32, u32);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let bytes = fs::read(img_path);
            diffuse_image = Ok(image::load_from_memory(&bytes.unwrap()).unwrap());
            dimensions = diffuse_image.as_ref().unwrap().dimensions();
            diffuse_rgba = diffuse_image.unwrap().to_rgba8();
        }

        use image::GenericImageView;

        let tex_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let tex = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            size: tex_size,
            mip_level_count: 1, // We'll talk about this a little later
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("diffuse_texture"),
            view_formats: &[],
        });
        self.gpu.queue.write_texture(
            // Tells wgpu where to copy the pixel data
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            // The actual pixel data
            &diffuse_rgba,
            // The layout of the texture
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            tex_size,
        );

        let tex_view = tex.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = self.gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&tex_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
                label: Some("diffuse_bind_group"),
            });

        bind_group
    }

    pub fn render_frame(
        &mut self,
        screen_descriptor: egui_wgpu::ScreenDescriptor,
        paint_jobs: Vec<egui::epaint::ClippedPrimitive>,
        textures_delta: egui::TexturesDelta,
    ) {
        for (id, image_delta) in &textures_delta.set {
            self.egui_renderer
                .update_texture(&self.gpu.device, &self.gpu.queue, *id, image_delta);
        }

        for id in &textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.egui_renderer.update_buffers(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        let surface_texture = self
            .gpu
            .surface
            .get_current_texture()
            .expect("Failed to get surface texture!");

        let surface_texture_view =
            surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    label: wgpu::Label::default(),
                    aspect: wgpu::TextureAspect::default(),
                    format: Some(self.gpu.surface_format),
                    dimension: None,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                    usage: None,
                });

        encoder.insert_debug_marker("Render scene");

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.19,
                        g: 0.24,
                        b: 0.42,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        let bind_group = self.create_texture(
            "./gopher.png",
            self.bind_group_layout.clone(),
        );

        render_pass.set_pipeline(&self.scene.pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);

        render_pass.set_vertex_buffer(0, self.scene.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.scene.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        render_pass.draw_indexed(0..3, 0, 0..1);
        render_pass.draw_indexed(3..6, 0, 0..1);

        self.egui_renderer.render(
            &mut render_pass.forget_lifetime(),
            &paint_jobs,
            &screen_descriptor,
        );

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
    }
}

pub struct Gpu {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub surface_format: wgpu::TextureFormat,
}

impl Gpu {
    pub fn aspect_ratio(&self) -> f32 {
        self.surface_config.width as f32 / self.surface_config.height.max(1) as f32
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn create_depth_texture(&self, width: u32, height: u32) -> wgpu::TextureView {
        let texture = self.device.create_texture(
            &(wgpu::TextureDescriptor {
                label: Some("Depth Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }),
        );
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
            usage: None,
        })
    }

    pub async fn new_async(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Self {
        let instance = wgpu::Instance::new(&InstanceDescriptor::default());
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to request adapter!");
        let (device, queue) = {
            log::info!("WGPU Adapter Features: {:#?}", adapter.features());
            adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("WGPU Device"),
                        memory_hints: wgpu::MemoryHints::default(),
                        required_features: wgpu::Features::default(),
                        #[cfg(not(target_arch = "wasm32"))]
                        required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                        #[cfg(all(target_arch = "wasm32", feature = "webgpu"))]
                        required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                        #[cfg(all(target_arch = "wasm32", feature = "webgl"))]
                        required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                            .using_resolution(adapter.limits()),
                    },
                    None,
                )
                .await
                .expect("Failed to request a device!")
        };

        let surface_capabilities = surface.get_capabilities(&adapter);

        let surface_format = surface_capabilities
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb()) // egui wants a non-srgb surface texture
            .unwrap_or(surface_capabilities.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: surface_capabilities.present_modes[0],
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        Self {
            surface,
            device,
            queue,
            surface_config,
            surface_format,
        }
    }
}

struct Scene {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub pipeline: wgpu::RenderPipeline,
}

impl Scene {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        bind_group_layout: &BindGroupLayout,
    ) -> Self {
        let vertex_buffer = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            },
        );
        let index_buffer = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("index Buffer"),
                contents: bytemuck::cast_slice(&INDICES),
                usage: wgpu::BufferUsages::INDEX,
            },
        );

        let pipeline = Self::create_pipeline(device, surface_format, &bind_group_layout);

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        bind_group_layout: &BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let shader_str = &String::from_utf8(include_bytes!("shader.wgsl").to_vec())
            .unwrap()
            .to_string();
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(shader_str)),
        });

        let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2].to_vec();
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vertex_main"),
                buffers: &[vertex_layout],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: Some(wgpu::IndexFormat::Uint32),
                front_face: wgpu::FrontFace::Cw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Renderer::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        })
    }
}
