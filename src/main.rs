// TODO:
// 1. GUI load file DONE!
// 2. Logic compile and run shaders DONE!
// 3. Parse shader to find uniforms DONE!
//      3.1. Compute memory layout of GUI struct DONE!
// 4. Autocreate UI to modify uniform DONE! (ish)
// 5. Watch shader file for change and autoreload DONE !
// 6. Parse comment options
//      6.1. Add texture loading
//      6.2. ...
// 7. Build up widget libraries
// 8. Gracefully handle bad app states

pub(crate) mod autogen_ui;
pub(crate) mod parser;
pub(crate) mod shader;
pub(crate) mod wgsl;

use autogen_ui::{EguiState, MyEvent};
use futures::{executor::ThreadPool, Future};
use notify::{DebouncedEvent, ReadDirectoryChangesWatcher, RecursiveMode, Watcher};
use shader::{ShaderFileBuf, Uniform, GUICONTROLLED_DEF};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use wgsl::Sized;
use winit::{
    event::{ElementState, Event, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowBuilder},
};

use crate::{
    parser::adjustment_for_safe_insert,
    shader::{ShaderFileBuilder, UniformChoice}, wgsl::PType,
};

fn create_shader_module(
    device: &wgpu::Device,
    shader_content: &ShaderFileBuf,
) -> Result<wgpu::ShaderModule, &'static str> {
    static mut VALID: bool = true;
    static mut ERROR: String = String::new();

    unsafe {
        VALID = true;
    }
    device.on_uncaptured_error(|e| {
        if let wgpu::Error::Validation { description, .. } = e {
            unsafe {
                VALID = false;
                ERROR = description;
            }
        }
    });
    let shader_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(shader_content.content.clone().into()),
    });

    if unsafe { VALID } {
        Ok(shader_module)
    } else {
        unsafe { Err(&ERROR) }
    }
}

fn create_render_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    config_format: wgpu::TextureFormat,
    shader_module: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader_module,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader_module,
            entry_point: "fs_main",
            targets: &[wgpu::ColorTargetState {
                format: config_format,
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
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    })
}

fn create_bind_group_layout(
    device: &wgpu::Device,
    std_uniform: &Uniform,
    gui_uniform: &Uniform,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std_uniform.dynamic_struct.slots.size() as _,
                    ),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        gui_uniform.dynamic_struct.slots.size() as _,
                    ),
                },
                count: None,
            },
        ],
        label: Some("my_bind_group_layout"),
    })
}

fn create_bind_group(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    std_gpu_buffer_handle: &wgpu::Buffer,
    gui_gpu_buffer_handle: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: std_gpu_buffer_handle.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: gui_gpu_buffer_handle.as_entire_binding(),
            },
        ],
        label: Some("my_bind_group"),
    })
}

fn create_file_watcher(
    shader_path: &Path,
    event_loop_proxy: &EventLoopProxy<MyEvent>,
) -> (impl Future<Output = ()>, ReadDirectoryChangesWatcher) {
    let watcher_event_loop_proxy = event_loop_proxy.clone();
    let (send, recv) = std::sync::mpsc::channel();
    let mut watcher = notify::watcher(send, Duration::from_millis(100u64)).unwrap();

    watcher
        .watch(shader_path, RecursiveMode::NonRecursive)
        .unwrap();

    (
        async move {
            loop {
                match recv.recv() {
                    Ok(DebouncedEvent::Write(p)) => watcher_event_loop_proxy
                        .send_event(MyEvent::ReloadShader(p))
                        .unwrap(),
                    Ok(DebouncedEvent::Remove(_)) | Ok(DebouncedEvent::Create(_)) => return,
                    Err(e) => println!("watch error: {:?}", e),
                    _ => {}
                }
            }
        },
        watcher,
    )
}

struct VulkanState {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    std_uniform: Uniform,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    start: Instant,
    ui: EguiState,
    file_watcher: ReadDirectoryChangesWatcher,
    old_shader_path: PathBuf,
}

impl VulkanState {
    async fn new(
        window: &Window,
        event_loop_proxy: &EventLoopProxy<MyEvent>,
        thread_pool: &ThreadPool,
    ) -> Self {
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
                None,
            )
            .await
            .unwrap();

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        surface.configure(&device, &config);

        let default_shader_path = std::path::PathBuf::from("shader/default.wgsl");

        let mut default_shader_builder =
            ShaderFileBuilder::new(&default_shader_path).expect("Default shader should be present");

        let std_uniform = default_shader_builder.uniform(&device, UniformChoice::StandardLib);
        let gui_uniform = default_shader_builder.uniform(&device, UniformChoice::GuiControlled);

        default_shader_builder.inject_content(GUICONTROLLED_DEF);

        for elem in &gui_uniform.dynamic_struct.slots {
            default_shader_builder.inject_content(&elem.generate_definition());
        }

        let default_shader_content = default_shader_builder.build();


        let bind_group_layout = create_bind_group_layout(&device, &std_uniform, &gui_uniform);
        let bind_group = create_bind_group(
            &device,
            &bind_group_layout,
            &std_uniform.gpu_buffer_handle,
            &gui_uniform.gpu_buffer_handle,
        );

        let shader_module = create_shader_module(&device, &default_shader_content)
            .expect("Render pipeline should never have a problem with the default shader");

        let render_pipeline =
            create_render_pipeline(&device, &bind_group_layout, config.format, &shader_module);

        let ui = EguiState::new(
            window,
            &device,
            config.format,
            event_loop_proxy,
            gui_uniform,
        );

        let (fw_future, file_watcher) = create_file_watcher(&default_shader_path, event_loop_proxy);

        thread_pool.spawn_ok(fw_future);
        Self {
            surface,
            device,
            queue,
            config,
            render_pipeline,
            std_uniform,
            bind_group,
            bind_group_layout,
            start: Instant::now(),
            ui,
            file_watcher,
            old_shader_path: default_shader_path,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.std_uniform
                .dynamic_struct
                .write_to_slot::<[u32; 2]>(1, &new_size.into()); // slot 1 is window_size
        }
    }

    fn handle_input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.std_uniform
                    .dynamic_struct
                    .write_to_slot::<[u32; 2]>(2, &(*position).into()); // slot 2 is mouse_pos
                true
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let mouse_pos = self
                    .std_uniform
                    .dynamic_struct
                    .read_from_slot_ref_mut::<[u32; 2]>(2)
                    .to_owned();
                self.std_uniform
                    .dynamic_struct
                    .write_to_slot::<[u32; 2]>(4, &mouse_pos);
                true
            }
            _ => false,
        }
    }

    fn update(&mut self) {
        self.ui
            .platform
            .update_time((Instant::now() - self.start).as_secs_f64());
        self.queue.write_buffer(
            &self.std_uniform.gpu_buffer_handle,
            0,
            self.std_uniform.dynamic_struct.buffer(),
        );

        self.queue.write_buffer(
            &self.ui.gui_uniform.gpu_buffer_handle,
            0,
            self.ui.gui_uniform.dynamic_struct.buffer(),
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
            // render_pass needs to be droped when finished with
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
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        self.ui.render(
            window,
            &view,
            &mut encoder,
            &self.device,
            &self.queue,
            &self.config,
        );

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn handle_user_event(
        &mut self,
        event: MyEvent,
        window: &Window,
        event_loop_proxy: &EventLoopProxy<MyEvent>,
        thread_pool: &futures::executor::ThreadPool,
    ) {
        match event {
            MyEvent::OpenFileDialog => {
                let dialog = rfd::AsyncFileDialog::new()
                    .add_filter("shader", &["wgsl"])
                    .set_parent(&window)
                    .set_directory(std::env::current_dir().expect("We should have a workdir"))
                    .pick_file();

                let event_loop_proxy_clone = event_loop_proxy.clone();
                thread_pool.spawn_ok(async move {
                    let new_shader_file = dialog.await;
                    let new_shader_path = new_shader_file.unwrap().path().to_owned();

                    event_loop_proxy_clone
                        .send_event(MyEvent::ReloadShader(new_shader_path))
                        .ok()
                        .expect("Event loop should send event...");
                });
            }
            MyEvent::ReloadShader(new_shader_path) => {
                let maybe_shader_content = ShaderFileBuilder::new(&new_shader_path);

                if let None = maybe_shader_content {
                    eprintln!("Shader wasn't found");
                    return;
                }

                let mut shader_builder = maybe_shader_content.unwrap();

                let candidate_uniform =
                    shader_builder.uniform(&self.device, UniformChoice::GuiControlled);

                shader_builder.inject_content(GUICONTROLLED_DEF);

                for elem in &candidate_uniform.dynamic_struct.slots {
                    shader_builder.inject_content(&elem.generate_definition());
                }

                let shader_content = shader_builder.build();

                let maybe_shader_module = create_shader_module(&self.device, &shader_content);

                if let Err(e) = maybe_shader_module {
                    eprintln!("{}", e);
                    return;
                }

                if candidate_uniform.dynamic_struct.slots
                    != self.ui.gui_uniform.dynamic_struct.slots
                {
                    self.ui.gui_uniform = candidate_uniform;
                }
                self.bind_group_layout =
                    create_bind_group_layout(&self.device, &self.std_uniform, &self.ui.gui_uniform);
                self.bind_group = create_bind_group(
                    &self.device,
                    &self.bind_group_layout,
                    &self.std_uniform.gpu_buffer_handle,
                    &self.ui.gui_uniform.gpu_buffer_handle,
                );
                self.render_pipeline = create_render_pipeline(
                    &self.device,
                    &self.bind_group_layout,
                    self.config.format,
                    &maybe_shader_module.unwrap(),
                );

                if new_shader_path != self.old_shader_path {
                    self.file_watcher
                        .unwatch(&self.old_shader_path)
                        .expect("Old shader path should be exist (was already used)");
                    self.file_watcher
                        .watch(&new_shader_path, RecursiveMode::NonRecursive)
                        .expect("Shader path should exist (was already validated)");

                    self.old_shader_path = new_shader_path;
                }
            }
        }
    }

    fn handle_window_event(&mut self, control_flow: &mut ControlFlow, event: &WindowEvent) {
        if !self.handle_input(event) {
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
                    self.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    self.resize(**new_inner_size);
                }
                _ => {}
            }
        }
    }
}

fn main() {
    // let fc = std::fs::read_to_string("shader/default.wgsl").unwrap();
    // let res = parser::parse_options(&fc);


    // std::process::exit(1);

    env_logger::init();
    let event_loop: EventLoop<MyEvent> = winit::event_loop::EventLoop::with_user_event();
    let thread_pool = futures::executor::ThreadPool::new().unwrap();

    let window = WindowBuilder::new()
        .with_visible(false)
        .with_title("Shadey - ur shader toy")
        .with_maximized(true)
        .build(&event_loop)
        .unwrap();

    let event_loop_proxy = event_loop.create_proxy();
    let mut state = pollster::block_on(VulkanState::new(&window, &event_loop_proxy, &thread_pool));

    window.set_visible(true);

    event_loop.run(move |event, _, control_flow| {
        state.ui.platform.handle_event(&event);
        state.std_uniform.dynamic_struct.write_to_slot(
            3, // slot 3 is time
            &(Instant::now() - state.start).as_secs_f32(),
        );

        match event {
            Event::WindowEvent {
                event: ref window_event,
                window_id,
            } if window_id == window.id() => {
                state.handle_window_event(control_flow, window_event);
            }
            Event::RedrawRequested(_) => {
                state.update();
                match state.render(&window) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => {
                        state.resize((state.config.height, state.config.width).into())
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::UserEvent(user_event) => {
                state.handle_user_event(user_event, &window, &event_loop_proxy, &thread_pool);
            }
            _ => {}
        }
    });
}
