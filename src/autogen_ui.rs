use std::{mem::MaybeUninit, time::Instant};

use egui_wgpu_backend::{epi::App, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use wgpu::{CommandEncoder, TextureView};
use winit::{dpi::PhysicalSize, event_loop::EventLoopProxy, window::Window};

use crate::wgsl::{DynamicStruct, PType, TType};

fn ui_f32(ui: &mut egui::Ui, gui_struct: &mut DynamicStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();
    let data = gui_struct.read_from_slot_ref_mut::<f32>(slot);
    ui.add(
        egui::widgets::Slider::new(data, 0.0..=1.0).text(identifier),
    );
}

fn ui_u32(ui: &mut egui::Ui, gui_struct: &mut DynamicStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();
    let data = gui_struct.read_from_slot_ref_mut::<u32>(slot);
    ui.add(egui::widgets::Slider::new(data, 0..=100).text(identifier));
}

fn ui_vec3f32(ui: &mut egui::Ui, gui_struct: &mut DynamicStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();

    let data = gui_struct.read_from_slot_ref_mut::<[f32; 3]>(slot);
    ui.horizontal(|ui| {
        ui.color_edit_button_rgb(data);
        ui.label(identifier);
    });
}

fn ui_vec4f32(ui: &mut egui::Ui, gui_struct: &mut DynamicStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();

    let data = gui_struct.read_from_slot_ref_mut::<[f32; 4]>(slot);
    ui.horizontal(|ui| {
        ui.color_edit_button_rgba_unmultiplied(data);
        ui.label(identifier);
    });
}

fn ui_vec3u32(ui: &mut egui::Ui, gui_struct: &mut DynamicStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();

    let mut data = gui_struct
        .read_from_slot_ref_mut::<[u32; 3]>(slot)
        .map(|x| x as f32 / 255.0);

    ui.horizontal(|ui| {
        ui.color_edit_button_rgb(&mut data);
        ui.label(identifier);
    });

    gui_struct.write_to_slot::<[u32; 3]>(slot, &data.map(|x| (x * 255.0) as u32));
}

fn ui_vec4u32(ui: &mut egui::Ui, gui_struct: &mut DynamicStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();

    let mut data = gui_struct
        .read_from_slot_ref_mut::<[u32; 4]>(slot)
        .map(|x| x as f32 / 255.0);
    ui.horizontal(|ui| {
        ui.color_edit_button_rgba_unmultiplied(&mut data);
        ui.label(identifier);
    });
    gui_struct.write_to_slot::<[u32; 4]>(slot, &data.map(|x| (x * 255.0) as u32));
}

pub fn generate_auto_ui(ctx: &egui::CtxRef, gui_struct: &mut DynamicStruct) {
    egui::SidePanel::right("autogen_ui").show(ctx, |ui| {
        use egui::*;
        trace!(ui);

        for i in 0..gui_struct.slots.len() {
            let actual_slot = i + 1;

            use PType::*;
            use TType::*;
            match &gui_struct.slots[i].typed {
                Scalar(F32) => ui_f32(ui, gui_struct, actual_slot),
                Scalar(U32) => ui_u32(ui, gui_struct, actual_slot),
                Vector(3, F32) => ui_vec3f32(ui, gui_struct, actual_slot),
                Vector(4, F32) => ui_vec4f32(ui, gui_struct, actual_slot),
                Vector(3, U32) => ui_vec3u32(ui, gui_struct, actual_slot),
                Vector(4, U32) => ui_vec4u32(ui, gui_struct, actual_slot),
                _ => {}
            }

            ui.add(widgets::Separator::default());
        }
    });
}

#[derive(Debug)]
pub enum MyEvent {
    OpenFileDialog,
    ReloadShader(String),
}

pub struct EguiState {
    pub platform: egui_winit_platform::Platform,
    render_pass: egui_wgpu_backend::RenderPass,
    previous_frame_time: Option<f32>,
    event_loop_proxy: EventLoopProxy<MyEvent>,
    pub gui_uniform: crate::Uniform,
}

impl egui_wgpu_backend::epi::App for EguiState {
    fn name(&self) -> &str {
        "Shadey"
    }

    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut egui_wgpu_backend::epi::Frame<'_>) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            use egui::*;
            trace!(ui);

            menu::bar(ui, |ui| {
                menu::menu(ui, "Shader", |ui| {
                    if ui.button("Open...").clicked() {
                        self.event_loop_proxy
                            .send_event(MyEvent::OpenFileDialog)
                            .ok()
                            .unwrap();
                    }
                });
            });
        });

        generate_auto_ui(ctx, &mut self.gui_uniform.dynamic_struct);
    }

    fn clear_color(&self) -> egui::Rgba {
        egui::Color32::TRANSPARENT.into()
    }
}

impl EguiState {
    pub fn new(
        window: &Window,
        device: &wgpu::Device,
        config_format: wgpu::TextureFormat,
        event_loop_proxy: &EventLoopProxy<MyEvent>,
        gui_uniform: crate::Uniform,
    ) -> Self {
        let PhysicalSize {
            height: physical_height,
            width: physical_width,
        } = window.inner_size();
        let platform = Platform::new(PlatformDescriptor {
            physical_width,
            physical_height,
            scale_factor: window.scale_factor(),
            font_definitions: egui::FontDefinitions::default(),
            style: Default::default(),
        });
        let egui_rpass = egui_wgpu_backend::RenderPass::new(device, config_format, 1);

        Self {
            platform,
            render_pass: egui_rpass,
            previous_frame_time: None,
            event_loop_proxy: event_loop_proxy.clone(),
            gui_uniform,
        }
    }

    pub fn render(
        &mut self,
        window: &Window,
        view: &TextureView,
        encoder: &mut CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
    ) {
        let start = Instant::now();
        self.platform.begin_frame();

        let egui_ctx = &self.platform.context();
        let frame = MaybeUninit::<&mut egui_wgpu_backend::epi::Frame>::uninit();
        /////////////////////////
        // Safety: We're not using the &mut frame in our implementation of update, therefore we invoke no undefined behaviour.
        /////////////////////////
        self.update(egui_ctx, unsafe { frame.assume_init() });
        let (_output, paint_commands) = self.platform.end_frame(Some(window));
        let paint_jobs = egui_ctx.tessellate(paint_commands);
        self.previous_frame_time = (Instant::now() - start).as_secs_f32().into();

        let screen_descriptor = ScreenDescriptor {
            physical_width: config.width,
            physical_height: config.height,
            scale_factor: window.scale_factor() as _,
        };
        self.render_pass
            .update_texture(device, queue, &egui_ctx.texture());
        self.render_pass.update_user_textures(device, queue);
        self.render_pass
            .update_buffers(device, queue, &paint_jobs, &screen_descriptor);
        self.render_pass
            .execute(encoder, view, &paint_jobs, &screen_descriptor, None)
            .unwrap();
    }
}
