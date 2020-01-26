use context::prelude::*;

use std::sync::{Arc, RwLock, RwLockReadGuard};

use cgmath::{vec2, vec4, InnerSpace};

use super::{
    light_field::LightField,
    rasterizer::{ColoredVertex, Rasterizer, TexturedVertex},
};

pub struct FeetRenderer {
    rasterizer: RwLock<Rasterizer>,

    // feet
    _feet: Arc<Image>,
    feet_descriptor_set: Arc<DescriptorSet>,
    feet_vertex_buffer: Arc<Buffer<TexturedVertex>>,

    // frustum outlines
    frustum_vertex_buffer: Arc<Buffer<ColoredVertex>>,
}

impl FeetRenderer {
    pub fn new(context: &Arc<Context>, light_fields: &[LightField]) -> VerboseResult<Self> {
        let feet_image = Image::from_file("feet.png")?
            .nearest_sampler()
            .build(context.device(), context.queue())?;

        let descriptor_layout = DescriptorSetLayout::builder()
            .add_layout_binding(
                0,
                VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER,
                VK_SHADER_STAGE_FRAGMENT_BIT,
                0,
            )
            .build(context.device().clone())?;

        let descriptor_pool = DescriptorPool::builder()
            .set_layout(descriptor_layout)
            .build(context.device().clone())?;

        let descriptor_set = DescriptorPool::prepare_set(&descriptor_pool).allocate()?;

        descriptor_set.update(&[DescriptorWrite::combined_samplers(0, &[&feet_image])]);

        let mut feet_vertex_data = Vec::new();
        let mut outline_vertex_data = Vec::new();

        let foot_size_m = 0.4;
        let length = foot_size_m / 2.0;

        for light_field in light_fields.iter() {
            // feet data
            let mut forward = light_field.direction;
            forward.y = 0.0;
            forward = forward.normalize_to(length);

            let mut right = light_field.right;
            right.y = 0.0;
            right = right.normalize_to(length);

            let mut center = light_field.center;
            center.y = 0.0;

            let left_top = center + forward - right;
            let right_top = center + forward + right;
            let left_bottom = center - forward - right;
            let right_bottom = center - forward + right;

            feet_vertex_data.push(TexturedVertex {
                position: left_top,
                uv: vec2(0.0, 0.0),
            });

            feet_vertex_data.push(TexturedVertex {
                position: left_bottom,
                uv: vec2(0.0, 1.0),
            });

            feet_vertex_data.push(TexturedVertex {
                position: right_bottom,
                uv: vec2(1.0, 1.0),
            });

            feet_vertex_data.push(TexturedVertex {
                position: right_bottom,
                uv: vec2(1.0, 1.0),
            });

            feet_vertex_data.push(TexturedVertex {
                position: right_top,
                uv: vec2(1.0, 0.0),
            });

            feet_vertex_data.push(TexturedVertex {
                position: left_top,
                uv: vec2(0.0, 0.0),
            });

            // outline data
            let frustum_edges = light_field.outlines();

            for (start, end) in frustum_edges.into_iter() {
                outline_vertex_data.push(ColoredVertex {
                    position: *start,
                    color: vec4(0.04, 0.8, 0.8, 1.0),
                });

                outline_vertex_data.push(ColoredVertex {
                    position: *end,
                    color: vec4(0.04, 0.8, 0.8, 1.0),
                })
            }
        }

        let feet_cpu_vertex_buffer = Buffer::builder()
            .set_memory_properties(
                VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                    | VK_MEMORY_PROPERTY_HOST_CACHED_BIT
                    | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT,
            )
            .set_usage(VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_VERTEX_BUFFER_BIT)
            .set_data(&feet_vertex_data)
            .build(context.device().clone())?;

        let outline_cpu_vertex_buffer = Buffer::builder()
            .set_memory_properties(
                VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                    | VK_MEMORY_PROPERTY_HOST_CACHED_BIT
                    | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT,
            )
            .set_usage(VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_VERTEX_BUFFER_BIT)
            .set_data(&outline_vertex_data)
            .build(context.device().clone())?;

        let command_buffer = context.render_core().allocate_primary_buffer()?;

        let feet_gpu_vertex_buffer =
            Buffer::into_device_local(feet_cpu_vertex_buffer, &command_buffer, context.queue())?;

        let outline_cpu_vertex_buffer =
            Buffer::into_device_local(outline_cpu_vertex_buffer, &command_buffer, context.queue())?;

        Ok(FeetRenderer {
            rasterizer: RwLock::new(Rasterizer::new(context)?),

            _feet: feet_image,
            feet_descriptor_set: descriptor_set,
            feet_vertex_buffer: feet_gpu_vertex_buffer,

            frustum_vertex_buffer: outline_cpu_vertex_buffer,
        })
    }

    pub fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        triangle_pipeline: &Arc<Pipeline>,
        line_pipeline: &Arc<Pipeline>,
        render_target: &RenderTarget,
        index: usize,
        transforms: VRTransformations,
    ) -> VerboseResult<()> {
        render_target.begin(command_buffer, VK_SUBPASS_CONTENTS_INLINE, index);

        // render outlines
        command_buffer.bind_pipeline(line_pipeline)?;
        command_buffer.bind_vertex_buffer(&self.frustum_vertex_buffer);
        command_buffer.draw_complete_single_instance(self.frustum_vertex_buffer.size() as u32);

        // render feet
        command_buffer.bind_pipeline(triangle_pipeline)?;
        command_buffer.bind_descriptor_sets_minimal(&[&self.feet_descriptor_set])?;
        command_buffer.push_constants(VK_SHADER_STAGE_VERTEX_BIT, &transforms)?;
        command_buffer.bind_vertex_buffer(&self.feet_vertex_buffer);
        command_buffer.draw_complete_single_instance(self.feet_vertex_buffer.size() as u32);

        render_target.end(command_buffer);

        Ok(())
    }

    pub fn rasterizer(&self) -> VerboseResult<RwLockReadGuard<'_, Rasterizer>> {
        Ok(self.rasterizer.read()?)
    }

    pub fn on_resize(&self, context: &Arc<Context>) -> VerboseResult<()> {
        *self.rasterizer.write()? = Rasterizer::new(context)?;

        Ok(())
    }
}
