use context::prelude::*;
use context::ContextObject;

use std::cell::RefCell;
use std::mem;
use std::ops::Deref;
use std::sync::Arc;

use cgmath::{vec3, vec4, Deg, Vector3};

#[derive(Clone, Copy, Debug)]
struct CoordinateVertex {
    position: Vector3<f32>,
    color: Vector3<f32>,
}

impl CoordinateVertex {
    fn input_descriptions() -> (
        Vec<VkVertexInputBindingDescription>,
        Vec<VkVertexInputAttributeDescription>,
    ) {
        let input_bindings = vec![VkVertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<Self>() as u32,
            inputRate: VK_VERTEX_INPUT_RATE_VERTEX,
        }];

        let input_attributes = vec![
            // position
            VkVertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: VK_FORMAT_R32G32B32_SFLOAT,
                offset: 0,
            },
            // uvs
            VkVertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: VK_FORMAT_R32G32B32_SFLOAT,
                offset: 12,
            },
        ];

        (input_bindings, input_attributes)
    }
}

pub struct CoordinateSystem {
    context: Arc<Context>,

    pipelines: RefCell<TargetMode<Arc<Pipeline>>>,

    vertex_buffer: Arc<Buffer<CoordinateVertex>>,
}

impl CoordinateSystem {
    pub fn new(
        context: &Arc<Context>,
        sample_count: VkSampleCountFlags,
        render_targets: &TargetMode<RenderTarget>,
        descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<Self> {
        let pipelines = Self::create_pipelines(context, sample_count, render_targets, descriptor)?;
        let vertex_buffer = Self::create_vertex_buffer(context)?;

        Ok(CoordinateSystem {
            context: context.clone(),

            pipelines: RefCell::new(pipelines),

            vertex_buffer,
        })
    }

    pub fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        descriptor_set: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        match self.pipelines.try_borrow()?.deref() {
            TargetMode::Single(pipeline) => {
                command_buffer.bind_pipeline(pipeline);
                command_buffer.bind_vertex_buffer(&self.vertex_buffer);
                command_buffer.bind_descriptor_sets_minimal(&[descriptor_set])?;
                command_buffer.draw_complete_single_instance(self.vertex_buffer.size() as u32);
            }
            TargetMode::Stereo(left_pipeline, right_pipeline) => {
                unimplemented!()

                // command_buffer.bind_pipeline(left_pipeline);
                // command_buffer.draw_complete_single_instance(self.vertex_buffer.size() as u32);

                // command_buffer.bind_pipeline(right_pipeline);
                // command_buffer.draw_complete_single_instance(self.vertex_buffer.size() as u32);
            }
        }

        Ok(())
    }

    pub fn resize(
        &self,
        sample_count: VkSampleCountFlags,
        render_targets: &TargetMode<RenderTarget>,
        descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        unimplemented!()
    }
}

// private helper
impl CoordinateSystem {
    #[inline]
    fn create_pipelines(
        context: &Arc<Context>,
        sample_count: VkSampleCountFlags,
        render_targets: &TargetMode<RenderTarget>,
        descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<TargetMode<Arc<Pipeline>>> {
        let vertex_shader = ShaderModule::from_slice(
            context.device().clone(),
            include_bytes!("line.vert.spv"),
            ShaderType::Vertex,
        )?;

        let fragment_shader = ShaderModule::from_slice(
            context.device().clone(),
            include_bytes!("line.frag.spv"),
            ShaderType::Fragment,
        )?;

        let pipeline_layout = PipelineLayout::new(context.device().clone(), &[descriptor], &[])?;

        let render_core = context.render_core();

        match render_targets {
            TargetMode::Single(render_target) => Ok(TargetMode::Single(Self::create_pipeline(
                context.device().clone(),
                vertex_shader,
                fragment_shader,
                sample_count,
                render_core.width(),
                render_core.height(),
                render_target.render_pass(),
                &pipeline_layout,
            )?)),
            TargetMode::Stereo(left_render_target, right_render_target) => {
                let left_pipeline = Self::create_pipeline(
                    context.device().clone(),
                    vertex_shader.clone(),
                    fragment_shader.clone(),
                    sample_count,
                    render_core.width(),
                    render_core.height(),
                    left_render_target.render_pass(),
                    &pipeline_layout,
                )?;

                let right_pipeline = Self::create_pipeline(
                    context.device().clone(),
                    vertex_shader,
                    fragment_shader,
                    sample_count,
                    render_core.width(),
                    render_core.height(),
                    right_render_target.render_pass(),
                    &pipeline_layout,
                )?;

                Ok(TargetMode::Stereo(left_pipeline, right_pipeline))
            }
        }
    }

    #[inline]
    fn create_pipeline(
        device: Arc<Device>,
        vertex_shader: Arc<ShaderModule>,
        fragment_shader: Arc<ShaderModule>,
        sample_count: VkSampleCountFlags,
        width: u32,
        height: u32,
        render_pass: &Arc<RenderPass>,
        pipeline_layout: &Arc<PipelineLayout>,
    ) -> VerboseResult<Arc<Pipeline>> {
        let (input_bindings, input_attributes) = CoordinateVertex::input_descriptions();

        Pipeline::new_graphics()
            .set_vertex_shader(vertex_shader, input_bindings, input_attributes)
            .set_fragment_shader(fragment_shader)
            .add_viewport(VkViewport {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: height as f32,
                minDepth: 0.0,
                maxDepth: 1.0,
            })
            .add_scissor(VkRect2D {
                offset: VkOffset2D { x: 0, y: 0 },
                extent: VkExtent2D {
                    width: width,
                    height: height,
                },
            })
            .default_multisample(sample_count)
            .default_rasterization(VK_CULL_MODE_NONE, VK_FRONT_FACE_COUNTER_CLOCKWISE)
            .default_color_blend(vec![VkPipelineColorBlendAttachmentState::default()])
            .default_depth_stencil(true, false)
            .input_assembly(VK_PRIMITIVE_TOPOLOGY_LINE_LIST, false)
            .build(device, pipeline_layout, render_pass, 0)
    }

    #[inline]
    fn create_vertex_buffer(
        context: &Arc<Context>,
    ) -> VerboseResult<Arc<Buffer<CoordinateVertex>>> {
        Buffer::new()
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_usage(VK_BUFFER_USAGE_VERTEX_BUFFER_BIT)
            .set_data(&[
                // normal in x direction, green
                CoordinateVertex {
                    position: vec3(0.0, 0.0, 0.0),
                    color: vec3(0.0, 1.0, 0.0),
                },
                CoordinateVertex {
                    position: vec3(1.0, 0.0, 0.0),
                    color: vec3(0.0, 1.0, 0.0),
                },
                // normal in y direction, red
                CoordinateVertex {
                    position: vec3(0.0, 0.0, 0.0),
                    color: vec3(1.0, 0.0, 0.0),
                },
                CoordinateVertex {
                    position: vec3(0.0, 1.0, 0.0),
                    color: vec3(1.0, 0.0, 0.0),
                },
                // normal in z direction, blue
                CoordinateVertex {
                    position: vec3(0.0, 0.0, 0.0),
                    color: vec3(0.0, 0.0, 1.0),
                },
                CoordinateVertex {
                    position: vec3(0.0, 0.0, 1.0),
                    color: vec3(0.0, 0.0, 1.0),
                },
            ])
            .build(context.device().clone())
    }
}
