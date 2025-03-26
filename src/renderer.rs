//! Core rendering logic, abstracted from the rest of WGPU.

use crate::{
    shared::{Bone, Shared, Texture, Vec2, Vertex},
    utils,
};
use wgpu::{BindGroup, BindGroupLayout, Device, Queue, RenderPass};

/// The `main` of this module.
pub fn render(render_pass: &mut RenderPass, device: &Device, shared: &mut Shared) {
    let mut temp_bones: Vec<Bone> = vec![];
    let mut i = 0;

    for b in &mut shared.armature.bones {
        if shared.selected_bone == i {
            // drag if holding left click
            if shared.mouse_left != -1 {
                if let Some(offset) = shared.mouse_bone_offset {
                    // move bone with mouse, keeping in mind their distance
                    let mouse_world = utils::screen_to_world_space(shared.mouse, shared.window);
                    b.pos = Vec2::new(mouse_world.x + offset.x, mouse_world.y + offset.y);
                } else {
                    // get initial distance between bone and mouse
                    let mouse_world = utils::screen_to_world_space(shared.mouse, shared.window);
                    shared.mouse_bone_offset =
                        Some(Vec2::new(b.pos.x - mouse_world.x, b.pos.y - mouse_world.y));
                }
            }
        }

        temp_bones.push(b.clone());
        i += 1;
    }

    i = 0;

    // using while loop to prevent borrow errors
    while i < temp_bones.len() {
        macro_rules! bone {
            () => {
                temp_bones[i]
            };
        }

        if bone!().tex_idx == usize::MAX {
            i += 1;
            continue;
        }

        // inherit from parent bone
        let mut p = Bone::default();
        if let Some(pp) = find_bone(&temp_bones, bone!().parent_id) {
            p = pp.clone();
        }

        // inherit position from parent
        bone!().pos.x += p.pos.x;
        bone!().pos.y += p.pos.y;

        let verts = rect_verts(
            &bone!(),
            &shared.armature.textures[bone!().tex_idx],
            shared.window.x / shared.window.y,
        );

        // render bone
        render_pass.set_bind_group(0, &shared.bind_groups[bone!().tex_idx], &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer(&verts, device).slice(..)); render_pass.set_index_buffer(
            index_buffer([0, 1, 2, 0, 1, 3].to_vec(), &device).slice(..),
            wgpu::IndexFormat::Uint32,
        );
        render_pass.draw_indexed(0..6, 0, 0..1);

        i += 1;
    }
}

/// Get bind group of a texture.
pub fn create_texture(
    pixels: Vec<u8>,
    dimensions: Vec2,
    textures: &mut Vec<crate::shared::Texture>,
    queue: &Queue,
    device: &Device,
    bind_group_layout: &BindGroupLayout,
) -> BindGroup {
    println!("test");
    // add to shared textures
    textures.push(crate::Texture {
        size: dimensions,
        pixels: pixels.to_vec(),
    });

    let tex_size = wgpu::Extent3d {
        width: dimensions.x as u32,
        height: dimensions.y as u32,
        depth_or_array_layers: 1,
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        size: tex_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        label: Some("diffuse_texture"),
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * dimensions.x as u32),
            rows_per_image: Some(dimensions.y as u32),
        },
        tex_size,
    );

    let tex_view = tex.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: None,
        ..Default::default()
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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

fn index_buffer(indices: Vec<u32>, device: &Device) -> wgpu::Buffer {
    wgpu::util::DeviceExt::create_buffer_init(
        device,
        &wgpu::util::BufferInitDescriptor {
            label: Some("index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        },
    )
}

fn vertex_buffer(vertices: &Vec<Vertex>, device: &Device) -> wgpu::Buffer {
    wgpu::util::DeviceExt::create_buffer_init(
        device,
        &wgpu::util::BufferInitDescriptor {
            label: Some("index Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        },
    )
}

/// Generate and return the vertices of a bone
///
/// Accounts for texture size and aspect ratio
fn rect_verts(bone: &Bone, tex: &Texture, aspect_ratio: f32) -> Vec<Vertex> {
    let hard_scale = 0.001;
    let vertices: Vec<Vertex> = vec![
        Vertex {
            pos: Vec2::new(
                (hard_scale * tex.size.x) / aspect_ratio + bone.pos.x,
                (hard_scale * tex.size.y) + bone.pos.y,
            ),
            uv: Vec2::new(1., 0.),
        },
        Vertex {
            pos: Vec2::new(
                (-hard_scale * tex.size.x) / aspect_ratio + bone.pos.x,
                (-hard_scale * tex.size.y) + bone.pos.y,
            ),
            uv: Vec2::new(0., 1.),
        },
        Vertex {
            pos: Vec2::new(
                (-hard_scale * tex.size.x) / aspect_ratio + bone.pos.x,
                (hard_scale * tex.size.y) + bone.pos.y,
            ),
            uv: Vec2::new(0., 0.),
        },
        Vertex {
            pos: Vec2::new(
                (hard_scale * tex.size.x) / aspect_ratio + bone.pos.x,
                (-hard_scale * tex.size.y) + bone.pos.y,
            ),
            uv: Vec2::new(1., 1.),
        },
    ];

    vertices
}

pub fn find_bone(bones: &Vec<Bone>, id: i32) -> Option<&Bone> {
    for b in bones {
        if b.id == id {
            return Some(&b);
        }
    }
    None
}
