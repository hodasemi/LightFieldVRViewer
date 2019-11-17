use context::prelude::*;

use std::sync::Arc;

use cgmath::vec3;

use super::debug_line_vertex::DebugLineVertex;

const AXIS_LENGTH: f32 = 5.0;

pub struct CoordinateSystem {
    pipeline: Arc<Pipeline>,
    vertex_buffer: Arc<Buffer<DebugLineVertex>>,
}

impl CoordinateSystem {
    pub fn new(
        context: &Arc<Context>,
        sample_count: VkSampleCountFlags,
        render_pass: &Arc<RenderPass>,
        descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<Self> {
        let pipeline =
            DebugLineVertex::create_pipeline(context, sample_count, render_pass, descriptor)?;
        let vertex_buffer = Self::create_vertex_buffer(context)?;

        Ok(CoordinateSystem {
            pipeline,
            vertex_buffer,
        })
    }

    pub fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        descriptor_set: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        command_buffer.bind_pipeline(&self.pipeline)?;
        command_buffer.bind_vertex_buffer(&self.vertex_buffer);
        command_buffer.bind_descriptor_sets_minimal(&[descriptor_set])?;
        command_buffer.draw_complete_single_instance(self.vertex_buffer.size() as u32);

        Ok(())
    }
}

// private helper
impl CoordinateSystem {
    #[inline]
    fn create_vertex_buffer(context: &Arc<Context>) -> VerboseResult<Arc<Buffer<DebugLineVertex>>> {
        Buffer::builder()
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_usage(VK_BUFFER_USAGE_VERTEX_BUFFER_BIT)
            .set_data(&[
                // normal in x direction, red
                DebugLineVertex {
                    position: vec3(0.0, 0.0, 0.0),
                    color: vec3(1.0, 0.0, 0.0),
                },
                DebugLineVertex {
                    position: vec3(AXIS_LENGTH, 0.0, 0.0),
                    color: vec3(1.0, 0.0, 0.0),
                },
                // normal in y direction, green
                DebugLineVertex {
                    position: vec3(0.0, 0.0, 0.0),
                    color: vec3(0.0, 1.0, 0.0),
                },
                DebugLineVertex {
                    position: vec3(0.0, AXIS_LENGTH, 0.0),
                    color: vec3(0.0, 1.0, 0.0),
                },
                // normal in z direction, blue
                DebugLineVertex {
                    position: vec3(0.0, 0.0, 0.0),
                    color: vec3(0.0, 0.0, 1.0),
                },
                DebugLineVertex {
                    position: vec3(0.0, 0.0, AXIS_LENGTH),
                    color: vec3(0.0, 0.0, 1.0),
                },
            ])
            .build(context.device().clone())
    }
}
