use context::prelude::*;

use super::light_field_frustum::LightFieldFrustum;

use cgmath::{vec2, Vector2, Vector3};

use std::collections::HashMap;
use std::sync::Arc;

const MAX_IMAGES_PER_LIGHT_FIELD: u32 = 1024;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightFieldVertex {
    position: Vector3<f32>,
    image_index: u32,
    uv: Vector2<f32>,
}

impl LightFieldVertex {
    fn create_quad(
        left_top: Vector3<f32>,
        left_bottom: Vector3<f32>,
        right_top: Vector3<f32>,
        right_bottom: Vector3<f32>,
        image_index: usize,
    ) -> [LightFieldVertex; 6] {
        [
            LightFieldVertex {
                position: left_top,
                image_index: image_index as u32,
                uv: vec2(0.0, 1.0),
            },
            LightFieldVertex {
                position: left_bottom,
                image_index: image_index as u32,
                uv: vec2(0.0, 0.0),
            },
            LightFieldVertex {
                position: right_bottom,
                image_index: image_index as u32,
                uv: vec2(1.0, 0.0),
            },
            LightFieldVertex {
                position: right_bottom,
                image_index: image_index as u32,
                uv: vec2(1.0, 0.0),
            },
            LightFieldVertex {
                position: right_top,
                image_index: image_index as u32,
                uv: vec2(1.0, 1.0),
            },
            LightFieldVertex {
                position: left_top,
                image_index: image_index as u32,
                uv: vec2(0.0, 1.0),
            },
        ]
    }

    pub fn vertex_input_info() -> (
        Vec<VkVertexInputBindingDescription>,
        Vec<VkVertexInputAttributeDescription>,
    ) {
        let input_bindings = vec![VkVertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<Self>() as u32,
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
            // image_index
            VkVertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: VK_FORMAT_R32_UINT,
                offset: 12,
            },
            // uvs
            VkVertexInputAttributeDescription {
                location: 2,
                binding: 0,
                format: VK_FORMAT_R32G32_SFLOAT,
                offset: 16,
            },
        ];

        (input_bindings, input_attributes)
    }
}

pub struct LightFieldRenderer {
    vertex_buffer: Arc<Buffer<LightFieldVertex>>,
    _images: Vec<Arc<Image>>,
    descriptor: Arc<DescriptorSet>,
}

impl LightFieldRenderer {
    pub fn new(
        context: &Arc<Context>,
        frustums: Vec<LightFieldFrustum>,
        image_data: Vec<(Vec<(Arc<Image>, f32)>, usize, usize)>,
    ) -> VerboseResult<LightFieldRenderer> {
        let mut sorted_frustums = HashMap::new();

        for frustum in frustums.iter() {
            sorted_frustums.insert(frustum.position(), frustum);
        }

        let mut image_collection = Vec::new();
        let mut vertices = Vec::new();

        for (images, x, y) in image_data.iter() {
            let frustum = sorted_frustums
                .get(&(*x, *y))
                .ok_or(format!("no frustum found at ({}, {})", x, y))?;

            for (image, depth) in images {
                let current_index = image_collection.len();
                image_collection.push(image.clone());

                let (left_top, left_bottom, right_top, right_bottom) =
                    frustum.get_corners_at_depth(*depth);

                vertices.extend(&LightFieldVertex::create_quad(
                    left_top,
                    left_bottom,
                    right_top,
                    right_bottom,
                    current_index,
                ));
            }
        }

        let vertex_buffer = Buffer::builder()
            .set_usage(VK_BUFFER_USAGE_VERTEX_BUFFER_BIT)
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_data(&vertices)
            .build(context.device().clone())?;

        let descriptor_pool = DescriptorPool::builder()
            .set_layout(Self::descriptor_layout(context.device().clone())?)
            .build(context.device().clone())?;

        let descriptor = DescriptorPool::prepare_set(&descriptor_pool).allocate()?;
        let image_refs: Vec<&Arc<Image>> = image_collection.iter().map(|i| i).collect();

        descriptor.update(&[DescriptorWrite::combined_samplers(0, &image_refs)]);

        Ok(LightFieldRenderer {
            vertex_buffer,
            _images: image_collection,
            descriptor,
        })
    }

    pub fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        transform_descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        command_buffer.bind_descriptor_sets_minimal(&[transform_descriptor, &self.descriptor])?;
        command_buffer.bind_vertex_buffer(&self.vertex_buffer);
        command_buffer.draw_complete_single_instance(self.vertex_buffer.size() as u32);

        Ok(())
    }

    pub fn descriptor_layout(device: Arc<Device>) -> VerboseResult<Arc<DescriptorSetLayout>> {
        DescriptorSetLayout::builder()
            .add_layout_binding(
                0,
                VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER,
                VK_SHADER_STAGE_FRAGMENT_BIT,
                VK_DESCRIPTOR_BINDING_VARIABLE_DESCRIPTOR_COUNT_BIT_EXT,
            )
            .change_descriptor_count(MAX_IMAGES_PER_LIGHT_FIELD)
            .build(device)
    }
}
