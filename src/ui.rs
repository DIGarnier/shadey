use std::{path::PathBuf, time::Instant};

use egui::FullOutput;
use egui_wgpu_backend::ScreenDescriptor;
use egui_winit_platform::{Platform, PlatformDescriptor};
use wgpu::{CommandEncoder, TextureView};
use winit::{
    dpi::PhysicalSize,
    event_loop::{EventLoop, EventLoopProxy},
    window::Window,
};

use super::{
    shader::Uniform,
    wgsl::{RuntimeStruct, PType, StructSlotOptions, TType},
};

fn make_f32(ui: &mut egui::Ui, gui_struct: &mut RuntimeStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();
    let range = gui_struct.slots[slot - 1]
        .options
        .as_ref()
        .map_or(0.0..=1.0, |x| {
            let StructSlotOptions::Slider { range } = x;
            range.to_owned()
        });
    let data = gui_struct.read_from_slot_ref_mut::<f32>(slot);
    ui.horizontal(|ui| {
        ui.add(egui::widgets::Slider::new(data, range));
        ui.label(identifier);
    });
}

fn make_u32(ui: &mut egui::Ui, gui_struct: &mut RuntimeStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();
    let range = gui_struct.slots[slot - 1]
        .options
        .as_ref()
        .map_or(0..=100, |x| {
            let StructSlotOptions::Slider { range } = x;
            let (s, e) = (*range.start(), *range.end());
            s as _..=e as _
        });
    let data = gui_struct.read_from_slot_ref_mut::<u32>(slot);
    ui.horizontal(|ui| {
        ui.add(egui::widgets::Slider::new(data, range));
        ui.label(identifier);
    });
}

fn make_vec3f32(ui: &mut egui::Ui, gui_struct: &mut RuntimeStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();
    let data = gui_struct.read_from_slot_ref_mut::<[f32; 3]>(slot);
    ui.horizontal(|ui| {
        ui.color_edit_button_rgb(data);
        ui.label(identifier);
    });
}

fn make_vec4f32(ui: &mut egui::Ui, gui_struct: &mut RuntimeStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();
    let data = gui_struct.read_from_slot_ref_mut::<[f32; 4]>(slot);
    ui.horizontal(|ui| {
        ui.color_edit_button_rgba_unmultiplied(data);
        ui.label(identifier);
    });
}

fn make_vec3u32(ui: &mut egui::Ui, gui_struct: &mut RuntimeStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();
    let mut data = gui_struct
        .read_from_slot_ref_mut::<[u32; 3]>(slot)
        .map(|x| x as f32 / 255.0);
    ui.horizontal(|ui| {
        ui.color_edit_button_rgb(&mut data);
        ui.label(identifier);
    });

    gui_struct.write_to_slot::<[u32; 3]>(slot, &data.map(|x| (x * 255.0) as _));
}

fn make_vec4u32(ui: &mut egui::Ui, gui_struct: &mut RuntimeStruct, slot: usize) {
    let identifier = gui_struct.slots[slot - 1].identifier.clone();
    let mut data = gui_struct
        .read_from_slot_ref_mut::<[u32; 4]>(slot)
        .map(|x| x as f32 / 255.0);
    ui.horizontal(|ui| {
        ui.color_edit_button_rgba_unmultiplied(&mut data);
        ui.label(identifier);
    });
    gui_struct.write_to_slot::<[u32; 4]>(slot, &data.map(|x| (x * 255.0) as _));
}

pub fn generate_auto_ui(ctx: &egui::Context, gui_struct: &mut RuntimeStruct) {
    egui::SidePanel::right("autogen_ui").show(ctx, |ui| {
        use egui::*;
        trace!(ui);

        for i in 0..gui_struct.slots.len() {
            let actual_slot = i + 1;

            use TType::*;
            use PType::*;
            match &gui_struct.slots[i].typed {
                Scalar(F32) => make_f32(ui, gui_struct, actual_slot),
                Scalar(U32) => make_u32(ui, gui_struct, actual_slot),
                Vector(3, F32) => make_vec3f32(ui, gui_struct, actual_slot),
                Vector(4, F32) => make_vec4f32(ui, gui_struct, actual_slot),
                Vector(3, U32) => make_vec3u32(ui, gui_struct, actual_slot),
                Vector(4, U32) => make_vec4u32(ui, gui_struct, actual_slot),
                _ => {}
            }

            ui.add(widgets::Separator::default());
        }
    });
}

#[derive(Debug)]
pub enum ShadeyEvent {
    OpenFileDialog,
    ReloadShader(PathBuf),
}

unsafe impl Sync for ShadeyEvent {}

pub struct Egui {
    pub platform: egui_winit_platform::Platform,
    render_pass: egui_wgpu_backend::RenderPass,
    previous_frame_time: Option<f32>,
    event_loop_proxy: EventLoopProxy<ShadeyEvent>,
    pub gui_uniform: Uniform,
    draw_right_panel: bool,
}

impl Egui {
    pub fn new(
        window: &Window,
        device: &wgpu::Device,
        config_format: wgpu::TextureFormat,
        event_loop: &EventLoop<ShadeyEvent>,
        gui_uniform: Uniform,
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
            event_loop_proxy: event_loop.create_proxy(),
            gui_uniform,
            draw_right_panel: true,
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
        self.update(egui_ctx);
        let FullOutput {shapes, ref textures_delta, ..} = self.platform.end_frame(Some(window));
        let paint_jobs = egui_ctx.tessellate(shapes);
        self.previous_frame_time = (Instant::now() - start).as_secs_f32().into();

        let screen_descriptor = ScreenDescriptor {
            physical_width: config.width,
            physical_height: config.height,
            scale_factor: window.scale_factor() as _,
        };
        self.render_pass.add_textures(device, queue, textures_delta).expect("add texture broke???");
        self.render_pass
            .update_buffers(device, queue, &paint_jobs, &screen_descriptor);
        self.render_pass
            .execute(encoder, view, &paint_jobs, &screen_descriptor, None)
            .unwrap();
    }

    fn update(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            use egui::*;

            menu::bar(ui, |ui| {
                menu::menu_button(ui, "Shader", |ui| {
                    if ui.button("Open...").clicked() {
                        self.event_loop_proxy
                            .send_event(ShadeyEvent::OpenFileDialog)
                            .ok()
                            .unwrap();
                    }
                });
                ui.add_space(ui.available_width() - 90.0);
                ui.checkbox(&mut self.draw_right_panel, " Draw Panel?");
            });
        });
        if self.draw_right_panel {
            generate_auto_ui(ctx, &mut self.gui_uniform.runtime_struct);
        }
    }
}
