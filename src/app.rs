use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use futures::executor::ThreadPool;
use notify_debouncer_full::{
    new_debouncer,
    notify::{self, ReadDirectoryChangesWatcher, RecursiveMode, Watcher},
    DebouncedEvent, Debouncer,
};
use wgpu::BindGroupLayout;

use winit::{
    event::{ElementState, Event, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowBuilder, WindowId},
};

use super::{
    shader::{ShaderFileBuf, ShaderFileBuilder, Uniform, UniformChoice, GUICONTROLLED_DEF},
    texture::Texture,
    ui::{Egui, ShadeyEvent},
    wgsl::Sized,
};

pub struct App {
    window: Window,
    thread_pool: ThreadPool,
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    std_uniform: Uniform,
    bind_groups: Vec<wgpu::BindGroup>,
    textures: Vec<Texture>,
    start_instant: Instant,
    ui: Egui,
    file_watcher: Debouncer<ReadDirectoryChangesWatcher, notify_debouncer_full::FileIdMap>,
    old_shader_path: PathBuf,
}

impl App {
    pub async fn new(event_loop: &EventLoop<ShadeyEvent>) -> Self {
        let window = WindowBuilder::new()
            .with_visible(false)
            .with_title("Shadey - ur shader toy")
            .with_maximized(true)
            .build(event_loop)
            .expect("Window to be created without problem");
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let surface = unsafe { instance.create_surface(&window).unwrap() };
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
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate,
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
        };
        surface.configure(&device, &config);

        let default_shader_path = std::path::PathBuf::from("shader/default.wgsl");

        let (default_shader_content, std_uniform, gui_uniform, textures) = {
            let mut default_shader_builder = ShaderFileBuilder::new(&default_shader_path)
                .expect("Default shader should be present");

            let std_uniform = default_shader_builder.uniform(&device, UniformChoice::StandardLib);
            let gui_uniform = default_shader_builder.uniform(&device, UniformChoice::GuiControlled);

            default_shader_builder.inject_content(GUICONTROLLED_DEF);
            for slot in &gui_uniform.dynamic_struct.slots {
                default_shader_builder.inject_content(&slot.generate_definition());
            }
            let textures = default_shader_builder.textures(&device, &queue);
            for (i, texture) in textures.iter().enumerate() {
                default_shader_builder.inject_content(&texture.generate_definition(i));
            }

            let default_shader_content = default_shader_builder.build();

            (default_shader_content, std_uniform, gui_uniform, textures)
        };

        let texture_bind_group_layouts =
            create_texture_bind_groups_layouts(&device, textures.len());

        let texture_bind_group =
            create_texture_bind_groups(&device, &textures, &texture_bind_group_layouts);

        let bind_group_layout = create_main_bind_group_layout(&device, &std_uniform, &gui_uniform);
        let bind_group_layouts = std::iter::once(bind_group_layout)
            .chain(texture_bind_group_layouts.into_iter())
            .collect::<Vec<_>>();

        let bind_group = create_main_bind_group(
            &device,
            &bind_group_layouts[0],
            &std_uniform.gpu_buffer_handle,
            &gui_uniform.gpu_buffer_handle,
        );

        let bind_groups = std::iter::once(bind_group)
            .chain(texture_bind_group.into_iter())
            .collect();

        let shader_module = create_shader_module(&device, &default_shader_content)
            .expect("Render pipeline should never have a problem with the default shader");

        let render_pipeline =
            create_render_pipeline(&device, &bind_group_layouts, config.format, &shader_module);

        let ui = Egui::new(&window, &device, config.format, event_loop, gui_uniform);

        let thread_pool =
            futures::executor::ThreadPool::new().expect("ThreadPool to be created without problem");
        let file_watcher = create_file_watcher(&default_shader_path, event_loop);

        window.set_visible(true);
        Self {
            window,
            thread_pool,
            surface,
            device,
            queue,
            config,
            render_pipeline,
            std_uniform,
            bind_groups,
            textures,
            start_instant: Instant::now(),
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

    pub fn handle_input(&mut self, event: &WindowEvent) -> bool {
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
                    .read_from_slot_ref_mut::<[u32; 2]>(2) // slot 2 is mouse_pos
                    .to_owned();
                self.std_uniform.dynamic_struct.write_to_slot(4, &mouse_pos); // slot 4 is toggle_mouse_pos
                true
            }
            _ => false,
        }
    }

    pub fn update(&mut self) {
        let time_elapsed = self.start_instant.elapsed().as_secs_f64();
        self.ui.platform.update_time(time_elapsed);

        self.std_uniform.dynamic_struct.write_to_slot(
            3, // slot 3 is time
            &(time_elapsed as f32),
        );

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

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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
                }
                .into()],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            for (i, group) in self.bind_groups.iter().enumerate() {
                render_pass.set_bind_group(i as _, group, &[]);
            }

            render_pass.draw(0..3, 0..1);
        }

        self.ui.render(
            &self.window,
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

    pub fn handle_user_event(
        &mut self,
        event: ShadeyEvent,
        event_loop_proxy: &EventLoopProxy<ShadeyEvent>,
    ) {
        match event {
            ShadeyEvent::OpenFileDialog => {
                let dialog = rfd::AsyncFileDialog::new()
                    .add_filter("shader", &["wgsl"])
                    .set_parent(&self.window)
                    .set_directory(std::env::current_dir().expect("We should have a workdir"))
                    .pick_file();

                let event_loop_proxy_clone = event_loop_proxy.clone();
                self.thread_pool.spawn_ok(async move {
                    let new_shader_file = dialog.await;
                    let new_shader_path = new_shader_file.unwrap().path().to_owned();

                    event_loop_proxy_clone
                        .send_event(ShadeyEvent::ReloadShader(new_shader_path))
                        .expect("Event loop should send event...");
                });
            }
            ShadeyEvent::ReloadShader(new_shader_path) => {
                let (shader_content, candidate_uniform, textures) = {
                    let maybe_shader_builder = ShaderFileBuilder::new(&new_shader_path);

                    if maybe_shader_builder.is_none() {
                        eprintln!("Shader wasn't found");
                        return;
                    }
                    let mut shader_builder = maybe_shader_builder.unwrap();

                    let candidate_uniform =
                        shader_builder.uniform(&self.device, UniformChoice::GuiControlled);

                    shader_builder.inject_content(GUICONTROLLED_DEF);
                    for slot in &candidate_uniform.dynamic_struct.slots {
                        shader_builder.inject_content(&slot.generate_definition());
                    }

                    let textures = shader_builder.textures(&self.device, &self.queue);
                    for (i, texture) in textures.iter().enumerate() {
                        shader_builder.inject_content(&texture.generate_definition(i));
                    }
                    let shader_content = shader_builder.build();

                    (shader_content, candidate_uniform, textures)
                };

                let texture_bind_group_layouts =
                    create_texture_bind_groups_layouts(&self.device, textures.len());
                let texture_bind_group = create_texture_bind_groups(
                    &self.device,
                    &textures,
                    &texture_bind_group_layouts,
                );
                let maybe_shader_module = create_shader_module(&self.device, &shader_content);

                if let Err(e) = maybe_shader_module {
                    eprintln!("{}", e);
                    return;
                }

                let shader_module = maybe_shader_module.unwrap();

                self.textures = textures;

                if candidate_uniform.dynamic_struct.slots
                    != self.ui.gui_uniform.dynamic_struct.slots
                {
                    self.ui.gui_uniform = candidate_uniform;
                }
                let bind_group_layout = create_main_bind_group_layout(
                    &self.device,
                    &self.std_uniform,
                    &self.ui.gui_uniform,
                );

                let bind_group_layouts: Vec<BindGroupLayout> = std::iter::once(bind_group_layout)
                    .chain(texture_bind_group_layouts.into_iter())
                    .collect();

                let bind_group = create_main_bind_group(
                    &self.device,
                    &bind_group_layouts[0],
                    &self.std_uniform.gpu_buffer_handle,
                    &self.ui.gui_uniform.gpu_buffer_handle,
                );

                self.bind_groups = std::iter::once(bind_group)
                    .chain(texture_bind_group.into_iter())
                    .collect();
                self.render_pipeline = create_render_pipeline(
                    &self.device,
                    &bind_group_layouts,
                    self.config.format,
                    &shader_module,
                );

                if new_shader_path != self.old_shader_path {
                    self.file_watcher
                        .watcher()
                        .unwatch(&self.old_shader_path)
                        .expect("Old shader path should be exist (was already used)");
                    self.file_watcher
                        .watcher()
                        .watch(&new_shader_path, RecursiveMode::NonRecursive)
                        .expect("Shader path should exist (was already validated)");

                    self.old_shader_path = new_shader_path;
                }
            }
        }
    }

    pub fn handle_window_event(
        &mut self,
        window_id: WindowId,
        control_flow: &mut ControlFlow,
        event: &WindowEvent,
    ) {
        if window_id != self.window.id() {
            return;
        }

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
                WindowEvent::Resized(physical_size) => self.resize(*physical_size),
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    self.resize(**new_inner_size)
                }
                _ => {}
            }
        }
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn platform_handle_event(&mut self, event: &Event<ShadeyEvent>) {
        self.ui.platform.handle_event(event);
    }
}

fn create_shader_module(
    device: &wgpu::Device,
    shader_content: &ShaderFileBuf,
) -> Result<wgpu::ShaderModule, &'static str> {
    static mut VALID: bool = true;
    static mut ERROR: String = String::new();

    unsafe {
        VALID = true;
    }
    device.on_uncaptured_error(Box::new(|e| {
        if let wgpu::Error::Validation { description, .. } = e {
            unsafe {
                VALID = false;
                ERROR = description;
            }
        }
    }));

    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(shader_content.as_ref().into()),
    });

    if unsafe { VALID } {
        Ok(shader_module)
    } else {
        unsafe { Err(&ERROR) }
    }
}

fn create_render_pipeline(
    device: &wgpu::Device,
    bind_group_layouts: &[wgpu::BindGroupLayout],
    config_format: wgpu::TextureFormat,
    shader_module: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &bind_group_layouts.iter().collect::<Vec<_>>()[..],
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
            }
            .into()],
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

fn create_main_bind_group_layout(
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
        label: Some("main_bind_group_layout"),
    })
}

fn create_main_bind_group(
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
        label: Some("main_bind_group"),
    })
}

fn create_texture_bind_groups(
    device: &wgpu::Device,
    textures: &Vec<Texture>,
    texture_bind_group_layouts: &[wgpu::BindGroupLayout],
) -> Vec<wgpu::BindGroup> {
    (0..textures.len())
        .map(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &texture_bind_group_layouts[i],
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&textures[i].view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&textures[i].sampler),
                    },
                ],
                label: Some("texture_bind_group"),
            })
        })
        .collect()
}

fn create_texture_bind_groups_layouts(
    device: &wgpu::Device,
    nb_required: usize,
) -> Vec<wgpu::BindGroupLayout> {
    (0..nb_required)
        .map(|i| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                label: Some(&format!("texture_bind_group_layout{}", i)),
            })
        })
        .collect()
}

fn create_file_watcher(
    shader_path: &Path,
    event_loop: &EventLoop<ShadeyEvent>,
) -> Debouncer<ReadDirectoryChangesWatcher, notify_debouncer_full::FileIdMap> {
    let watcher_event_loop_proxy = event_loop.create_proxy();
    let mut debouncer = new_debouncer(
        Duration::from_millis(100u64),
        None,
        move |res: Result<Vec<DebouncedEvent>, Vec<notify_debouncer_full::notify::Error>>| match res
        {
            Ok(events) => {
                let Some(debounced_event) = events.last() else {
                    return;
                };

                use notify::*;
                if let Event {
                    kind: EventKind::Modify(_),
                    paths,
                    ..
                } = &debounced_event.event
                {
                    watcher_event_loop_proxy
                        .send_event(ShadeyEvent::ReloadShader(paths.last().unwrap().clone()))
                        .unwrap()
                }
            }
            Err(e) => println!("watch error: {:?}", e),
        },
    )
    .unwrap();

    debouncer
        .watcher()
        .watch(shader_path, RecursiveMode::NonRecursive)
        .unwrap();

    debouncer
}
