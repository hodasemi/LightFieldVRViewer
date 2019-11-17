use context::prelude::*;

use std::sync::Arc;

use cgmath::vec3;

use super::debug_line_vertex::DebugLineVertex;
use crate::light_field::light_field_frustum::LightFieldFrustum;

#[derive(Debug)]
pub struct FrustumRenderer {
    pipeline: Arc<Pipeline>,
    buffer: Arc<Buffer<DebugLineVertex>>,
}

impl FrustumRenderer {
    pub fn new(
        context: &Arc<Context>,
        sample_count: VkSampleCountFlags,
        render_pass: &Arc<RenderPass>,
        descriptor: &Arc<DescriptorSet>,
        frustums: &[LightFieldFrustum],
    ) -> VerboseResult<Self> {
        let pipeline =
            DebugLineVertex::create_pipeline(context, sample_count, render_pass, descriptor)?;
        let buffer = Self::create_vertex_buffer(context, frustums)?;

        Ok(FrustumRenderer { pipeline, buffer })
    }

    pub fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        descriptor_set: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        command_buffer.bind_pipeline(&self.pipeline)?;
        command_buffer.bind_vertex_buffer(&self.buffer);
        command_buffer.bind_descriptor_sets_minimal(&[descriptor_set])?;
        command_buffer.draw_complete_single_instance(self.buffer.size() as u32);

        Ok(())
    }
}

impl FrustumRenderer {
    fn create_vertex_buffer(
        context: &Arc<Context>,
        frustums: &[LightFieldFrustum],
    ) -> VerboseResult<Arc<Buffer<DebugLineVertex>>> {
        let color = vec3(0.0, 1.0, 1.0);

        let mut data = Vec::new();

        for frustum in frustums {
            // top left
            data.push(DebugLineVertex {
                position: frustum.left_top.center,
                color,
            });
            data.push(DebugLineVertex {
                position: frustum.left_top.center + frustum.left_top.direction,
                color,
            });

            // top right
            data.push(DebugLineVertex {
                position: frustum.right_top.center,
                color,
            });
            data.push(DebugLineVertex {
                position: frustum.right_top.center + frustum.right_top.direction,
                color,
            });

            // bottom left
            data.push(DebugLineVertex {
                position: frustum.left_bottom.center,
                color,
            });
            data.push(DebugLineVertex {
                position: frustum.left_bottom.center + frustum.left_bottom.direction,
                color,
            });

            // bottom right
            data.push(DebugLineVertex {
                position: frustum.right_bottom.center,
                color,
            });
            data.push(DebugLineVertex {
                position: frustum.right_bottom.center + frustum.right_bottom.direction,
                color,
            });
        }

        Buffer::builder()
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_usage(VK_BUFFER_USAGE_VERTEX_BUFFER_BIT)
            .set_data(&data)
            .build(context.device().clone())
    }
}
