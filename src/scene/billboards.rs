// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crossbeam_queue::SegQueue;
use glsl_layout::AsStd140;
use na::{Matrix3, Matrix4, Vector3};
use rand::distributions::{Distribution, Uniform as RandUniform};
use rayon::prelude::*;
use std::{convert::TryInto as _, future::Future, mem, sync::Arc};
use winit::dpi::{PhysicalPosition, PhysicalSize};

use super::uniform::Uniform;
use super::{DEFAULT_FORMAT, DEPTH_FORMAT, ID_FORMAT};
use crate::command_encoder::CommandEncoder;

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(C)]
pub struct Point {
    pub pos: Vector3<f32>,
    pub kind: u32,
}

unsafe impl bytemuck::Zeroable for Point {}
unsafe impl bytemuck::Pod for Point {}

#[derive(Debug, Copy, Clone, PartialEq, AsStd140)]
struct Uniforms {
    world_mx: glsl_layout::mat4,
    projection_mx: glsl_layout::mat4,
    inv_view_mx: glsl_layout::mat3,
    cursor: glsl_layout::uvec2,
}

pub struct Billboards {
    render_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: Uniform<Uniforms>,

    depth_texture: wgpu::Texture,
    id_texture: wgpu::Texture,
    cursor_id_buffer_queue: Arc<SegQueue<wgpu::Buffer>>,
    current_size: PhysicalSize<u32>,

    /// These are temporary.
    /// Eventually, the buffer of points will be generated by a compute shader
    /// or this pipeline will be replaced by a mesh shader pipeline.
    point_buffer: wgpu::Buffer,
    num_points: usize,
}

impl Billboards {
    pub fn new(device: &wgpu::Device, size: PhysicalSize<u32>) -> Self {
        let num_points = 100;

        let point_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            size: (mem::size_of::<Point>() * num_points) as u64,
            usage: wgpu::BufferUsage::STORAGE,
            mapped_at_creation: true,
            label: None,
        });

        {
            let buffer_slice = point_buffer.slice(..);

            let mut writable_view = buffer_slice.get_mapped_range_mut();

            let pos_die = RandUniform::from(-10.0..10.0);
            let kind_die = RandUniform::from(0..=1);

            writable_view[..]
                .par_chunks_mut(mem::size_of::<Point>())
                .for_each_init(
                    || rand::thread_rng(),
                    |rng, chunk| {
                        chunk.copy_from_slice(bytemuck::bytes_of(&Point {
                            pos: Vector3::new(
                                pos_die.sample(rng),
                                pos_die.sample(rng),
                                pos_die.sample(rng),
                            ),
                            kind: kind_die.sample(rng),
                        }))
                    },
                );
        }

        point_buffer.unmap();

        create_billboards(device, size, point_buffer, num_points)
    }

    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        world_mx: Matrix4<f32>,
        projection_mx: Matrix4<f32>,
        inv_view_mx: Matrix3<f32>,
        cursor: PhysicalPosition<u32>,
    ) {
        let uniforms = Uniforms {
            world_mx: Into::<[[f32; 4]; 4]>::into(world_mx).into(),
            projection_mx: Into::<[[f32; 4]; 4]>::into(projection_mx).into(),
            inv_view_mx: Into::<[[f32; 3]; 3]>::into(inv_view_mx).into(),
            cursor: Into::<[u32; 2]>::into(cursor).into(),
        };

        // TODO: Replace with just `queue.write_buffer` instead of this layer of abstraction.
        self.uniform_buffer.set(queue, uniforms);
    }

    pub fn resize(&mut self, device: &wgpu::Device, size: PhysicalSize<u32>) {
        self.depth_texture = create_depth_texture(device, size);
        self.id_texture = create_id_texture(device, size);
        self.current_size = size;
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: wgpu::TextureView) {
        let depth_view = self.depth_texture.create_default_view();
        let id_view = self.id_texture.create_default_view();

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[
                wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &target,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color::WHITE,
                },
                wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &id_view,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color::TRANSPARENT,
                },
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                attachment: &depth_view,
                depth_load_op: wgpu::LoadOp::Clear,
                depth_store_op: wgpu::StoreOp::Store,
                clear_depth: 1.0,
                stencil_load_op: wgpu::LoadOp::Clear,
                stencil_store_op: wgpu::StoreOp::Store,
                clear_stencil: 1,
                depth_read_only: false,
                stencil_read_only: true,
            }),
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(
            0..(self.num_points * 3) // See shaders/billboard.vert.
                .try_into()
                .expect("too many points to draw"),
            0..1,
        );
    }

    pub fn get_mouseover_id(
        &self,
        device: &wgpu::Device,
        encoder: &mut CommandEncoder,
        cursor_pos: PhysicalPosition<u32>,
    ) -> impl Future<Output = Option<u32>> {
        let cursor_id_buffer_queue = Arc::clone(&self.cursor_id_buffer_queue);

        let cursor_id_buffer = cursor_id_buffer_queue.pop().unwrap_or_else(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                size: mem::size_of::<u32>() as u64,
                usage: wgpu::BufferUsage::MAP_READ | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: false,
                label: None,
            })
        });

        encoder.copy_texture_to_buffer(
            wgpu::TextureCopyView {
                texture: &self.id_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: cursor_pos.x,
                    y: cursor_pos.y,
                    z: 0,
                },
            },
            wgpu::BufferCopyView {
                buffer: &cursor_id_buffer,
                layout: wgpu::TextureDataLayout {
                    offset: 0,
                    bytes_per_row: self.current_size.width * mem::size_of::<u32>() as u32,
                    rows_per_image: self.current_size.height,
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth: 0,
            },
        );

        let on_submit = encoder.on_submit();

        async move {
            on_submit.await;

            let id = {
                let buffer_slice = cursor_id_buffer.slice(..);
                buffer_slice
                    .map_async(wgpu::MapMode::Read)
                    .await
                    .expect("unable to map id buffer");

                let view = buffer_slice.get_mapped_range();
                u32::from_le_bytes(view[..mem::size_of::<u32>()].try_into().unwrap())
            };

            cursor_id_buffer.unmap();
            cursor_id_buffer_queue.push(cursor_id_buffer);

            if id != 0 {
                Some(id - 1)
            } else {
                None
            }
        }
    }
}

fn create_billboards(
    device: &wgpu::Device,
    size: PhysicalSize<u32>,
    point_buffer: wgpu::Buffer,
    num_points: usize,
) -> Billboards {
    let vert_shader = include_shader_binary!("billboard.vert");
    let frag_shader = include_shader_binary!("billboard.frag");

    let vert_module = device.create_shader_module(vert_shader);
    let frag_module = device.create_shader_module(frag_shader);

    let uniform_buffer = Uniform::new(device);

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                ..Default::default()
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::StorageBuffer {
                    dynamic: false,
                    readonly: true,
                },
                ..Default::default()
            },
        ],
        label: None,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        bindings: &[
            wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(uniform_buffer.buffer_view()),
            },
            wgpu::Binding {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(point_buffer.slice(..)),
            },
        ],
        label: None,
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_group_layout],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        layout: &pipeline_layout,
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vert_module,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &frag_module,
            entry_point: "main",
        }),
        rasterization_state: Some(wgpu::RasterizationStateDescriptor {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        }),
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        color_states: &[
            wgpu::ColorStateDescriptor {
                format: DEFAULT_FORMAT,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            },
            wgpu::ColorStateDescriptor {
                format: ID_FORMAT,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            },
        ],
        depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil_front: Default::default(),
            stencil_back: Default::default(),
            stencil_read_mask: !0,
            stencil_write_mask: !0,
        }),
        // Ignored, since we're not using a vertex buffer.
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: &[],
        },
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    });

    let depth_texture = create_depth_texture(device, size);
    let id_texture = create_id_texture(device, size);

    Billboards {
        render_pipeline,
        bind_group,
        uniform_buffer,

        depth_texture,
        id_texture,
        current_size: size,
        cursor_id_buffer_queue: Arc::new(SegQueue::new()),

        point_buffer,
        num_points,
    }
}

fn create_depth_texture(device: &wgpu::Device, size: PhysicalSize<u32>) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        label: None,
    })
}

fn create_id_texture(device: &wgpu::Device, size: PhysicalSize<u32>) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: ID_FORMAT,
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::COPY_SRC,
        label: None,
    })
}
