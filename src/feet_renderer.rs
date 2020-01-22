use context::prelude::*;

use std::sync::{Arc, RwLock, RwLockReadGuard};

use cgmath::{vec2, InnerSpace};

use super::{
    light_field::LightField,
    rasterizer::{Rasterizer, TexturedVertex},
};

pub struct FeetRenderer {
    rasterizer: RwLock<Rasterizer>,

    _feet: Arc<Image>,
    descriptor_set: Arc<DescriptorSet>,
    vertex_buffer: Arc<Buffer<TexturedVertex>>,
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

        let mut vertex_data = Vec::new();

        let foot_size_m = 0.4;
        let length = foot_size_m / 2.0;

        for light_field in light_fields.iter() {
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

            vertex_data.push(TexturedVertex {
                position: left_top,
                uv: vec2(0.0, 0.0),
            });

            vertex_data.push(TexturedVertex {
                position: left_bottom,
                uv: vec2(0.0, 1.0),
            });

            vertex_data.push(TexturedVertex {
                position: right_bottom,
                uv: vec2(1.0, 1.0),
            });

            vertex_data.push(TexturedVertex {
                position: right_bottom,
                uv: vec2(1.0, 1.0),
            });

            vertex_data.push(TexturedVertex {
                position: right_top,
                uv: vec2(1.0, 0.0),
            });

            vertex_data.push(TexturedVertex {
                position: left_top,
                uv: vec2(0.0, 0.0),
            });
        }

        let cpu_vertex_buffer = Buffer::builder()
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_usage(VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_VERTEX_BUFFER_BIT)
            .set_data(&vertex_data)
            .build(context.device().clone())?;

        let command_buffer = context.render_core().allocate_primary_buffer()?;

        let gpu_vertex_buffer =
            Buffer::into_device_local(cpu_vertex_buffer, &command_buffer, context.queue())?;

        Ok(FeetRenderer {
            rasterizer: RwLock::new(Rasterizer::new(context)?),

            _feet: feet_image,
            descriptor_set,
            vertex_buffer: gpu_vertex_buffer,
        })
    }

    pub fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        pipeline: &Arc<Pipeline>,
        render_target: &RenderTarget,
        index: usize,
        transforms: VRTransformations,
    ) -> VerboseResult<()> {
        render_target.begin(command_buffer, VK_SUBPASS_CONTENTS_INLINE, index);

        command_buffer.bind_pipeline(pipeline)?;
        command_buffer.bind_descriptor_sets_minimal(&[&self.descriptor_set])?;
        command_buffer.push_constants(VK_SHADER_STAGE_VERTEX_BIT, &transforms)?;
        command_buffer.bind_vertex_buffer(&self.vertex_buffer);
        command_buffer.draw_complete_single_instance(self.vertex_buffer.size() as u32);

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
