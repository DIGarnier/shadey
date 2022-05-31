use std::path::Path;

use wgpu::util::DeviceExt;

use crate::{
    parser,
    wgsl::{DynamicStruct, StructSlot},
};

pub const GUICONTROLLED_DEF: &str = "[[group(0), binding(1)]]\n\
var<uniform> _gui: GuiControlled;";

pub struct ShaderFileBuilder {
    pub content: String,
    pub inject_pos: usize,
}

pub struct ShaderFileBuf {
    pub content: String,
}

impl ShaderFileBuilder {
    pub fn new(shader_path: &Path) -> Option<Self> {
        let std_content =
            std::fs::read_to_string("shader/std.wgsl").expect("Shader std lib couldn't be found");
        let shader_content = std::fs::read_to_string(shader_path).ok()?;

        let candidate_inject_pos = std_content.len()
            + parser::adjustment_for_safe_insert(
                &shader_content,
                UniformChoice::GuiControlled.into(),
            )?;
        let content = std_content + &shader_content[..];

        if candidate_inject_pos > content.len() {
            return None;
        }

        Self {
            content,
            inject_pos: candidate_inject_pos,
        }
        .into()
    }

    pub fn inject_content(&mut self, content: &str) {
        self.content
            .insert_str(self.inject_pos, &format!("{}\n", content));
        self.inject_pos += content.len() + 1;
    }

    pub fn build(self) -> ShaderFileBuf {
        ShaderFileBuf {
            content: self.content,
        }
    }

    pub fn uniform(&self, device: &wgpu::Device, uniform_choice: UniformChoice) -> Uniform {
        Uniform::new(device, self, uniform_choice.into())
    }
}

pub enum UniformChoice {
    StandardLib,
    GuiControlled,
}

impl From<UniformChoice> for &str {
    fn from(choice: UniformChoice) -> Self {
        match choice {
            UniformChoice::StandardLib => "StdUniform",
            UniformChoice::GuiControlled => "GuiControlled",
        }
    }
}

pub struct Uniform {
    pub dynamic_struct: DynamicStruct,
    pub gpu_buffer_handle: wgpu::Buffer,
}

impl Uniform {
    fn new(
        device: &wgpu::Device,
        shader_content: &ShaderFileBuilder,
        uniform_typename: &str,
    ) -> Self {
        let (_, slots) = parser::parse_struct_named(&shader_content.content, uniform_typename)
            .expect(&format!(
                "Problem with typename {} in shader",
                uniform_typename
            ));

        Self::from_slots(device, slots, uniform_typename)
    }

    fn from_slots(device: &wgpu::Device, slots: Vec<StructSlot>, uniform_typename: &str) -> Self {
        let dynamic_struct = DynamicStruct::new(slots);
        let gpu_buffer_handle = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} Buffer", uniform_typename)),
            contents: dynamic_struct.buffer(),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            dynamic_struct,
            gpu_buffer_handle,
        }
    }
}