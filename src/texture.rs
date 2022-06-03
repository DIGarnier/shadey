
use std::fs;

use image::GenericImageView;

use crate::{parser::ShaderOptions};

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub name: String,
}


impl Texture {
    pub fn generate_definition(&self, group_id: usize) -> String {
        format!(
            "[[group({group_id}), binding(0)]] \n\
            var t_diffuse{group_id}: texture_2d<f32>; \n\
            [[group({group_id}), binding(1)]] \n\
            var s_diffuse{group_id}: sampler; \n\
            fn texture_{name}(vx: vec2<f32>) -> vec4<f32> \n\
            {{ return textureSample(t_diffuse{group_id}, s_diffuse{group_id}, flipy(vx));}}"
            ,
            group_id=group_id+1,
            name=self.name
        )
    }
}

impl Texture {
    pub fn from_shader_option(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        options: &ShaderOptions,
    ) -> Option<Self> {
        if let ShaderOptions::Texture { path, .. } = options {
            let texture_content = fs::read(path).ok()?;
            return Self::from_bytes(device, queue, &texture_content, options)
        }

        None
    }
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        options: &ShaderOptions,
    ) -> Option<Self> {
        let img = image::load_from_memory(bytes).ok()?;
        Self::from_image(device, queue, &img, options)
    }


    // yoinked from https://sotrh.github.io/learn-wgpu/beginner/tutorial5-textures/#cleaning-things-up
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        options: &ShaderOptions,
    ) -> Option<Self> {
        if let ShaderOptions::Texture {
            name,
            u_addr_mode,
            v_addr_mode,
            w_addr_mode,
            ..
        } = options
        {
            let rgba = img.to_rgba8();
            let dimensions = img.dimensions();

            let size = wgpu::Extent3d {
                width: dimensions.0,
                height: dimensions.1,
                depth_or_array_layers: 1,
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(name),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            });
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    aspect: wgpu::TextureAspect::All,
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                },
                &rgba,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.0),
                    rows_per_image: std::num::NonZeroU32::new(dimensions.1),
                },
                size,
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: u_addr_mode.unwrap_or(wgpu::AddressMode::ClampToEdge),
                address_mode_v: v_addr_mode.unwrap_or(wgpu::AddressMode::ClampToEdge),
                address_mode_w: w_addr_mode.unwrap_or(wgpu::AddressMode::ClampToEdge),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

            return Some(Self {
                texture,
                view,
                sampler,
                name: name.to_owned(),
            });
        }

        None
    }
}
