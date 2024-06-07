use std::{
    iter,
    sync::{Arc, Mutex},
};

use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::game::GameState;

pub const CIRCLE_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

mod texture;

/// A circle that will be rendered to the screen.
///
/// Although the circle generally uses normalized device coordinates, it will
/// adjust for aspect ratio.
#[derive(Debug)]
pub struct Cell {
    /// Where the circle will be drawn on the screen, between 0 and 1, where 1
    /// is the top-left and formatted as x, y. This is the position of the
    /// top-left corner of it's bounding box.
    pub location: [f32; 2],
}

impl Cell {
    fn as_instance(&self, _radius: f32) -> Instance {
        let normalized_location = [
            self.location[0] * 2.0 - 1.0,
            -1.0 * (self.location[1] * 2.0 - 1.0),
        ];
        let center = [normalized_location[0], normalized_location[1]];
        Instance {
            offset: normalized_location,
            center,
        }
    }
}

fn circle_vertices(radius: f32) -> [Vertex; 6] {
    [
        Vertex {
            position: [-radius, -radius, 0.0],
            tex_coords: [0.0, 1.0],
        },
        Vertex {
            position: [radius, -radius, 0.0],
            tex_coords: [1.0, 1.0],
        },
        Vertex {
            position: [radius, radius, 0.0],
            tex_coords: [1.0, 0.0],
        },
        Vertex {
            position: [-radius, -radius, 0.0],
            tex_coords: [0.0, 1.0],
        },
        Vertex {
            position: [radius, radius, 0.0],
            tex_coords: [1.0, 0.0],
        },
        Vertex {
            position: [-radius, radius, 0.0],
            tex_coords: [0.0, 0.0],
        },
    ]
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct Instance {
    offset: [f32; 2],
    center: [f32; 2],
}

impl Instance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Instance>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We'll have to reassemble the mat4 in the shader.
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials, we'll
                    // be using 2, 3, and 4, for Vertex. We'll start at slot 5, not conflict with them later
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
    fn new_bg() -> [Vertex; 6] {
        [
            Vertex {
                position: [-1.0, -1.0, 0.0],
                tex_coords: [0.0, 1.0],
            },
            Vertex {
                position: [1.0, -1.0, 0.0],
                tex_coords: [1.0, 1.0],
            },
            Vertex {
                position: [1.0, 1.0, 0.0],
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [-1.0, -1.0, 0.0],
                tex_coords: [0.0, 1.0],
            },
            Vertex {
                position: [1.0, 1.0, 0.0],
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [-1.0, 1.0, 0.0],
                tex_coords: [0.0, 0.0],
            },
        ]
    }
}

/// A struct that holds the core of the render state.
struct RenderCore<'a> {
    surface: Arc<wgpu::Surface<'a>>,
    device: Arc<wgpu::Device>,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

struct BuffersAndGroups {
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_buffer_capacity: u64,

    #[allow(dead_code)]
    radius_buffer: wgpu::Buffer,
    radius_bind_group: wgpu::BindGroup,

    #[allow(dead_code)]
    color_buffer: wgpu::Buffer,
    color_bind_group: wgpu::BindGroup,

    res_buffer: wgpu::Buffer,
    res_bind_group: wgpu::BindGroup,

    #[allow(dead_code)]
    diffuse_texture: texture::Texture,
    diffuse_bind_group: wgpu::BindGroup,

    #[allow(dead_code)]
    bg_texture: texture::Texture,
    bg_texture_bind_group: wgpu::BindGroup,

    #[allow(dead_code)]
    offset_buffer: wgpu::Buffer,
    offset_bind_group: wgpu::BindGroup,
    bg_vertex_buffer: wgpu::Buffer,
}

mod gui;

pub struct RenderState<'a> {
    core: RenderCore<'a>,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    window: Arc<Window>,
    num_vertices: u32,
    circles: Vec<Cell>,
    grid_size: f32,
    rsc: BuffersAndGroups,
    bg_render_pipeline: wgpu::RenderPipeline,
    egui: gui::GuiState,
}

impl<'a> RenderState<'a> {
    /// Create a new RenderState, ready for rendering.
    ///
    /// # Args
    /// window:
    /// A reference to a winit window, to which we will be rendering
    ///
    /// grid_size:
    /// The size of the grid. This is from 0 to 1 * the height of the viewport
    pub async fn new(
        window: Arc<Window>,
        grid_size: f32,
        start_capacity: u64,
        game_state: Arc<Mutex<GameState>>,
    ) -> RenderState<'a> {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits {
                        max_bind_groups: 5,
                        ..Default::default()
                    },
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an Srgb surface texture. Using a different
        // one will result all the colors comming out darker. If you want to support non
        // Srgb surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let res_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Resolution Buffer"),
            contents: bytemuck::cast_slice(&[size.width as f32, size.height as f32]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let res_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Resolution Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let res_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Resolution Bind Group"),
            layout: &res_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: res_buffer.as_entire_binding(),
            }],
        });

        let radius_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Radius Buffer"),
            contents: bytemuck::cast_slice(&[grid_size]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let radius_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Radius Bind Group Layout"),
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
            });
        let radius_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Radius Bind Group"),
            layout: &radius_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: radius_buffer.as_entire_binding(),
            }],
        });

        let color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Color Buffer"),
            contents: bytemuck::cast_slice(&CIRCLE_COLOR),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let color_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Color Bind Group Layout"),
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
            });
        let color_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Color Bind Group"),
            layout: &color_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: color_buffer.as_entire_binding(),
            }],
        });

        let instances: Vec<Instance> = Vec::new();

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            // size: std::mem::size_of::<Instance>() as u64 * 80u64,
            size: std::mem::size_of::<Instance>() as u64 * start_capacity,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        queue.write_buffer(&instance_buffer, 0, bytemuck::cast_slice(&instances));

        let diffuse_bytes = include_bytes!("../../rsc/live.png");
        let diffuse_texture =
            texture::Texture::from_bytes(&device, &queue, diffuse_bytes, "live.png").unwrap();

        let texture_bind_group_layout =
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
                label: Some("texture_bind_group_layout"),
            });

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let bg_texture_bytes = include_bytes!("../../rsc/dead.png");
        let bg_texture =
            texture::Texture::from_bytes(&device, &queue, bg_texture_bytes, "dead.png").unwrap();
        let bg_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&bg_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&bg_texture.sampler),
                },
            ],
            label: Some("bg_texture_bind_group"),
        });

        let vertices = circle_vertices(grid_size);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let offset_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Offset Buffer"),
            contents: bytemuck::cast_slice(&[0.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let offset_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("offset_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let offset_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("offset_bind_group"),
            layout: &offset_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: offset_buffer.as_entire_binding(),
            }],
        });

        let bg_vertices = Vertex::new_bg();
        let bg_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("BG Vertex Buffer"),
            contents: bytemuck::cast_slice(&bg_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // let depth_texture =
        //     texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        // Loads the shader at runtime. Change this for prod, but it makes shader
        // changes faster.
        let shader_string = include_str!("./shader.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_string.into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &res_bind_group_layout,
                    &radius_bind_group_layout,
                    &color_bind_group_layout,
                    &texture_bind_group_layout,
                    &offset_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc(), Instance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::POLYGON_MODE_LINE
                // or Features::POLYGON_MODE_POINT
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            // If the pipeline will be used with a multiview render pass, this
            // indicates how many array layers the attachments will have.
            multiview: None,
        });

        let bg_shader_string = include_str!("./bg.wgsl");
        let bg_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BG Shader"),
            source: wgpu::ShaderSource::Wgsl(bg_shader_string.into()),
        });
        let bg_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("BG Render Pipeline Layout"),
                bind_group_layouts: &[
                    &offset_bind_group_layout,
                    &radius_bind_group_layout,
                    &texture_bind_group_layout,
                    &res_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });
        let bg_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BG Render Pipeline"),
            layout: Some(&bg_render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &bg_shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &bg_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
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
        });

        let surface = Arc::new(surface);
        let device = Arc::new(device);

        let core = RenderCore {
            surface,
            device,
            queue,
            config,
        };

        let bag = BuffersAndGroups {
            vertex_buffer,
            instance_buffer,
            instance_buffer_capacity: start_capacity,

            radius_buffer,
            radius_bind_group,

            color_buffer,
            color_bind_group,

            res_buffer,
            res_bind_group,

            diffuse_bind_group,
            diffuse_texture,

            offset_buffer,
            offset_bind_group,

            bg_vertex_buffer,

            bg_texture,
            bg_texture_bind_group,
        };

        let egui = gui::GuiState::new(
            size,
            Arc::clone(&window),
            core.device.clone(),
            surface_format,
            game_state,
        );

        Self {
            core,
            size,
            render_pipeline,
            window,
            num_vertices: vertices.len() as u32,
            circles: Vec::new(),
            grid_size,
            rsc: bag,
            bg_render_pipeline,
            egui,
        }
    }

    /// Update the circles to be rendered.
    ///
    /// Automatically allocates new buffers when their capacity is insufficient
    pub fn update_circles(&mut self, circles: Vec<Cell>) {
        self.circles = circles;
        let new_instances = self
            .circles
            .iter()
            .map(|c| c.as_instance(self.grid_size))
            .collect::<Vec<_>>();

        let instance_count = new_instances.len();
        let new_size = (instance_count as f32 * 1.5) as u64;

        if instance_count as u64 > self.rsc.instance_buffer_capacity {
            let instance_buffer = self.core.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Instance Buffer"),
                // size: std::mem::size_of::<Instance>() as u64 * 80u64,
                size: std::mem::size_of::<Instance>() as u64 * new_size,
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            self.core
                .queue
                .write_buffer(&instance_buffer, 0, bytemuck::cast_slice(&new_instances));
            self.rsc.instance_buffer_capacity = new_size;
            self.rsc.instance_buffer = instance_buffer;
        } else {
            self.core.queue.write_buffer(
                &self.rsc.instance_buffer,
                0,
                bytemuck::cast_slice(&new_instances),
            );
        }
    }

    pub fn window(&self) -> Arc<Window> {
        self.window.clone()
    }

    pub fn update_offset(&mut self, new_offset: vec2::Vector2<f32>) {
        let offset: [f32; 2] = new_offset.into();
        self.core
            .queue
            .write_buffer(&self.rsc.offset_buffer, 0, bytemuck::cast_slice(&offset));
    }

    #[allow(dead_code)]
    pub fn change_grid_size(&self, new: f32) {
        if new <= 0.0 {
            return;
        }
        let vertices = circle_vertices(new);
        self.core
            .queue
            .write_buffer(&self.rsc.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        self.core
            .queue
            .write_buffer(&self.rsc.radius_buffer, 0, bytemuck::cast_slice(&[new]));
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.size = new_size;
        self.core.config.width = new_size.width;
        self.core.config.height = new_size.height;
        self.core
            .surface
            .configure(&self.core.device, &self.core.config);

        self.core.queue.write_buffer(
            &self.rsc.res_buffer,
            0 as wgpu::BufferAddress,
            bytemuck::cast_slice(&[new_size.width as f32, new_size.height as f32]),
        );
    }

    pub fn reconfigure(&mut self) {
        self.resize(self.size);
    }

    pub fn handle_event<T>(&mut self, event: &winit::event::Event<T>) -> bool {
        self.egui.handle_event(event)
    }

    pub fn update(&mut self) {}

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.core.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.core
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        {
            let mut first_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("BG Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            first_render_pass.set_pipeline(&self.bg_render_pipeline);

            first_render_pass.set_bind_group(0, &self.rsc.offset_bind_group, &[]);
            first_render_pass.set_bind_group(1, &self.rsc.radius_bind_group, &[]);
            first_render_pass.set_bind_group(2, &self.rsc.bg_texture_bind_group, &[]);
            first_render_pass.set_bind_group(3, &self.rsc.res_bind_group, &[]);

            first_render_pass.set_vertex_buffer(0, self.rsc.bg_vertex_buffer.slice(..));

            first_render_pass.draw(0..6, 0..1);
        }
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.rsc.res_bind_group, &[]);
            render_pass.set_bind_group(1, &self.rsc.radius_bind_group, &[]);
            render_pass.set_bind_group(2, &self.rsc.color_bind_group, &[]);
            render_pass.set_bind_group(3, &self.rsc.diffuse_bind_group, &[]);
            render_pass.set_bind_group(4, &self.rsc.offset_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.rsc.vertex_buffer.slice(..));

            render_pass.set_vertex_buffer(1, self.rsc.instance_buffer.slice(..));

            render_pass.draw(0..self.num_vertices, 0..self.circles.len() as _);
        }

        let (encoder, egui_tdelta) =
            self.egui
                .render(&self.core.config, &self.core.queue, &view, encoder);

        self.core.queue.submit(iter::once(encoder.finish()));

        output.present();

        self.egui.remove_textures(egui_tdelta);

        Ok(())
    }
}
