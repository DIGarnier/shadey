use std::{
    iter,
    time::{Instant, SystemTime},
};

use egui::FontDefinitions;
use egui_wgpu_backend::{
    epi::{
        backend::{AppOutput, FrameBuilder},
        App, IntegrationInfo,
    },
    ScreenDescriptor,
};
use egui_winit_platform::{Platform, PlatformDescriptor};
use wgpu::{util::DeviceExt, CommandEncoder, TextureView};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InfoUniform {
    window_size: [u32; 2],
    mouse: [u32; 2],
    time: f32,
}

impl InfoUniform {
    fn update_window_size(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.window_size = new_size.into();
    }

    fn update_mouse(&mut self, new_mouse: winit::dpi::PhysicalPosition<f64>) {
        self.mouse = [new_mouse.x as u32, new_mouse.y as u32];
    }

    fn update_time(&mut self, timer: SystemTime) {
        self.time = timer.elapsed().expect("Time update failed!").as_secs_f32();
    }
}

enum MyEvent {
    RequestRedraw,
}

struct ExampleRepaintSignal(std::sync::Mutex<winit::event_loop::EventLoopProxy<MyEvent>>);

impl egui_wgpu_backend::epi::RepaintSignal for ExampleRepaintSignal {
    fn request_repaint(&self) {
        self.0
            .lock()
            .unwrap()
            .send_event(MyEvent::RequestRedraw)
            .ok();
    }
}

struct MyUi {
    platform: Platform,
    render_pass: egui_wgpu_backend::RenderPass,
    demo_app: egui_demo_lib::WrapApp,
    repaint_signal: std::sync::Arc<ExampleRepaintSignal>,
    previous_frame_time: Option<f32>,
}

impl MyUi {
    fn new(
        window: &Window,
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        event_loop: &EventLoop<MyEvent>,
    ) -> Self {
        let platform = Platform::new(PlatformDescriptor {
            physical_width: window.inner_size().width as u32,
            physical_height: window.inner_size().height as u32,
            scale_factor: window.scale_factor(),
            font_definitions: FontDefinitions::default(),
            style: Default::default(),
        });

        let egui_rpass = egui_wgpu_backend::RenderPass::new(device, config.format, 1);
        let demo_app = egui_demo_lib::WrapApp::default();

        let repaint_signal = std::sync::Arc::new(ExampleRepaintSignal(std::sync::Mutex::new(
            event_loop.create_proxy(),
        )));

        Self {
            platform,
            render_pass: egui_rpass,
            demo_app,
            repaint_signal,
            previous_frame_time: None,
        }
    }

    fn render(
        &mut self,
        window: &Window,
        view: &TextureView,
        encoder: &mut CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
    ) {
        let egui_start = Instant::now();
        self.platform.begin_frame();
        let mut app_output = AppOutput::default();
        let mut frame = FrameBuilder {
            info: IntegrationInfo {
                name: "A frame",
                web_info: None,
                cpu_usage: self.previous_frame_time,
                native_pixels_per_point: Some(window.scale_factor() as _),
                prefer_dark_mode: None,
            },
            tex_allocator: &mut self.render_pass,
            output: &mut app_output,
            repaint_signal: self.repaint_signal.clone(),
        }
        .build();

        self.demo_app.update(&self.platform.context(), &mut frame);
        let (_output, paint_commands) = self.platform.end_frame(Some(window));
        let paint_jobs = self.platform.context().tessellate(paint_commands);
        let frame_time = (Instant::now() - egui_start).as_secs_f64() as f32;
        self.previous_frame_time = Some(frame_time);

        let screen_descriptor = ScreenDescriptor {
            physical_width: config.width,
            physical_height: config.height,
            scale_factor: window.scale_factor() as f32,
        };
        self.render_pass
            .update_texture(device, queue, &self.platform.context().texture());
        self.render_pass.update_user_textures(device, queue);
        self.render_pass
            .update_buffers(device, queue, &paint_jobs, &screen_descriptor);

        self.render_pass
            .execute(encoder, view, &paint_jobs, &screen_descriptor, None)
            .unwrap();
    }
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    window_size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    info_uniform: InfoUniform,
    info_buffer: wgpu::Buffer,
    info_bind_group: wgpu::BindGroup,
    timer: SystemTime,
    ui: MyUi,
}

impl State {
    async fn new(window: &Window, event_loop: &EventLoop<MyEvent>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::VULKAN);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::default(),
                    limits: wgpu::Limits::default(),
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        surface.configure(&device, &config);

        let info_uniform = InfoUniform {
            window_size: size.into(),
            mouse: [0, 0],
            time: 0.,
        };
        let info_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Info Buffer"),
            contents: bytemuck::cast_slice(&[info_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let info_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("info_bind_group_layout"),
            });
        let info_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &info_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: info_buffer.as_entire_binding(),
            }],
            label: Some("info_bind_group"),
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&info_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                clamp_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        });

        let timer = std::time::SystemTime::now();

        let ui = MyUi::new(window, &device, &config, event_loop);

        Self {
            surface,
            device,
            queue,
            window_size: size,
            config,
            render_pipeline,
            info_uniform,
            info_buffer,
            info_bind_group,
            timer,
            ui,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.window_size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.info_uniform.update_window_size(new_size);
        }
    }

    #[allow(unused_variables)]
    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.info_uniform.update_mouse(*position);
                true
            }
            _ => false,
        }
    }

    fn update(&mut self) {
        self.ui
            .platform
            .update_time(self.timer.elapsed().unwrap().as_secs_f64());
        self.queue.write_buffer(
            &self.info_buffer,
            0,
            bytemuck::cast_slice(&[self.info_uniform]),
        );
    }

    fn render(&mut self, window: &Window) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.info_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        {
            self.ui.render(
                window,
                &view,
                &mut encoder,
                &self.device,
                &self.queue,
                &self.config,
            );
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn main() {
    env_logger::init();
    let event_loop: EventLoop<MyEvent> = winit::event_loop::EventLoop::with_user_event();
    let window = WindowBuilder::new()
        .with_title("Shadey - ur shadertoy")
        .build(&event_loop)
        .unwrap();

    let mut state = pollster::block_on(State::new(&window, &event_loop));

    event_loop.run(move |event, _, control_flow| {
        state.ui.platform.handle_event(&event);
        state.info_uniform.update_time(state.timer);
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    state: ElementState::Pressed,
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                },
                            ..
                        } => *control_flow = ControlFlow::Exit,
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            state.resize(**new_inner_size);
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawRequested(_) => {
                state.update();
                match state.render(&window) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.window_size),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => {}
        }
    });
}
