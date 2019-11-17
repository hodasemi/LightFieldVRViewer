use context::prelude::*;

use std::mem;
use std::sync::Arc;

use cgmath::Vector3;

#[derive(Clone, Copy, Debug)]
pub struct DebugLineVertex {
    pub position: Vector3<f32>,
    pub color: Vector3<f32>,
}

impl DebugLineVertex {
    pub fn input_descriptions() -> (
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

    #[inline]
    pub fn create_pipeline(
        context: &Arc<Context>,
        sample_count: VkSampleCountFlags,
        render_pass: &Arc<RenderPass>,
        descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<Arc<Pipeline>> {
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

        let (input_bindings, input_attributes) = DebugLineVertex::input_descriptions();

        Pipeline::new_graphics()
            .set_vertex_shader(vertex_shader, input_bindings, input_attributes)
            .set_fragment_shader(fragment_shader)
            .add_viewport(VkViewport {
                x: 0.0,
                y: 0.0,
                width: render_core.width() as f32,
                height: render_core.height() as f32,
                minDepth: 0.0,
                maxDepth: 1.0,
            })
            .add_scissor(VkRect2D {
                offset: VkOffset2D { x: 0, y: 0 },
                extent: VkExtent2D {
                    width: render_core.width(),
                    height: render_core.height(),
                },
            })
            .default_multisample(sample_count)
            .default_rasterization(VK_CULL_MODE_NONE, VK_FRONT_FACE_COUNTER_CLOCKWISE)
            .default_color_blend(vec![VkPipelineColorBlendAttachmentState::default()])
            .default_depth_stencil(true, false)
            .input_assembly(VK_PRIMITIVE_TOPOLOGY_LINE_LIST, false)
            .build(context.device().clone(), &pipeline_layout, render_pass, 0)
    }
}
