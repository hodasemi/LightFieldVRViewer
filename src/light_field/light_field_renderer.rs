use context::prelude::*;

use super::{light_field_frustum::LightFieldFrustum, ranges::Ranges};

use cgmath::{vec2, InnerSpace, Vector2, Vector3};
use image::{ImageBuffer, Pixel, Rgba};

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

struct PlaneImageRatios {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl PlaneImageRatios {
    fn is_inside(&self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

struct PlaneImage {
    image: ImageBuffer<Rgba<u8>, Vec<u8>>,
    frustum: (usize, usize),
    depth_values: Ranges,
}

struct DisparityPlane {
    images: Vec<PlaneImage>,
    disparity_index: usize,
}

impl LightFieldRenderer {
    pub fn new(
        context: &Arc<Context>,
        mut frustums: Vec<LightFieldFrustum>,
        mut image_data: Vec<(
            Vec<(ImageBuffer<Rgba<u8>, Vec<u8>>, usize, Ranges)>,
            usize,
            usize,
        )>,
        frustum_extent: (usize, usize),
        baseline: f32,
    ) -> VerboseResult<LightFieldRenderer> {
        // move data from vector to internal more practical formats with:
        //      while Some(...) = vector.pop() {}
        // this moves ownership into new structures

        // create a map for frustums
        let mut sorted_frustums = HashMap::new();

        while let Some(frustum) = frustums.pop() {
            sorted_frustums.insert(frustum.position(), frustum);
        }

        // sort all images by their respective disparity layers
        let mut disparity_planes: Vec<DisparityPlane> = Vec::new();

        while let Some((mut images, x, y)) = image_data.pop() {
            while let Some((image, disparity_index, depth_values)) = images.pop() {
                // create plane image
                let plane_image = PlaneImage {
                    image,
                    frustum: (x, y),
                    depth_values,
                };

                // search for disparity index
                match disparity_planes
                    .iter()
                    .position(|plane| plane.disparity_index == disparity_index)
                {
                    // if we can find the disparity layer, just add the plane image
                    Some(index) => disparity_planes[index].images.push(plane_image),

                    // if we couldn't find the disparity layer, add layer and image
                    None => disparity_planes.push(DisparityPlane {
                        images: vec![plane_image],
                        disparity_index,
                    }),
                }
            }
        }

        // sort ascending by disparity index
        disparity_planes.sort_by(|lhs, rhs| lhs.disparity_index.cmp(&rhs.disparity_index));

        let mut image_collection = Vec::new();
        let mut vertices = Vec::new();

        while let Some(mut disparity_plane) = disparity_planes.pop() {
            // calculate average depth of disparity layer
            let mut total_depth = 0.0;
            let mut total_count = 0;

            for image in disparity_plane.images.iter() {
                if let Some(average) = image.depth_values.weighted_average(0.01) {
                    total_depth += average;
                    total_count += 1;
                }
            }

            let layer_depth = (total_depth / total_count as f64) as f32;

            println!("\nlayer index: {}", disparity_plane.disparity_index);
            println!("{:.2}", layer_depth);

            // TODO:
            // (1) [x] find corner frustums (assuming a rectangle)
            // (2) [x] get image extent
            // (3) [x] correctly place all images inside this plane
            // (4) [ ] offline interpolation of images
            // (5) [x] add result to vulkan buffer and descriptor

            // (1) find corner frustums
            let left_top_frustum = &sorted_frustums[&(0, 0)];
            let left_bottom_frustum = &sorted_frustums[&(0, frustum_extent.1 - 1)];
            let right_top_frustum = &sorted_frustums[&(frustum_extent.0 - 1, 0)];
            let right_bottom_frustum =
                &sorted_frustums[&(frustum_extent.0 - 1, frustum_extent.1 - 1)];

            // (2) get image extent
            let left_top = left_top_frustum.get_corners_at_depth(layer_depth).0;
            let left_bottom = left_bottom_frustum.get_corners_at_depth(layer_depth).1;
            let right_top = right_top_frustum.get_corners_at_depth(layer_depth).2;
            let right_bottom = right_bottom_frustum.get_corners_at_depth(layer_depth).3;

            // (3) placing images into that plane

            // since all cameras have the same aperture and baseline given in the parameters file of every light field
            // the length of every side in total = the length of the side of a camera + baseline * (cameras - 1)

            let total_width = (left_top - right_top).magnitude();
            let total_height = (left_top - left_bottom).magnitude();

            let mut image_locations = Vec::new();

            while let Some(image) = disparity_plane.images.pop() {
                let (left_top, left_bottom, right_top, _) =
                    sorted_frustums[&image.frustum].get_corners_at_depth(layer_depth);

                let width = (left_top - right_top).magnitude();
                let height = (left_top - left_bottom).magnitude();
                let x = image.frustum.0 as f32 * baseline;
                let y = image.frustum.1 as f32 * baseline;

                let left_ratio = x / total_width;
                let right_ratio = (x + width) / total_width;
                let top_ratio = y / height;
                let bottom_ratio = (y + height) / total_height;

                let ratios = PlaneImageRatios {
                    left: left_ratio,
                    right: right_ratio,
                    top: top_ratio,
                    bottom: bottom_ratio,
                };

                let center_x = x + width / 2.0;
                let center_y = y + height / 2.0;

                image_locations.push((image.image, ratios, vec2(center_x, center_y)));
            }

            // TODO: size of image
            // maybe: size of base image + ratio of the missing part ?
            let image_width = 1024;
            let image_height = 1024;

            // (4) offline interpolation of disparity plane image
            let mut disparity_plane_image: ImageBuffer<Rgba<u8>, Vec<u8>> =
                ImageBuffer::new(image_width, image_height);

            for (x, y, plane_pixel) in disparity_plane_image.enumerate_pixels_mut() {
                // calculate x and y ratio of this pixel in the disparity plane image

                // consider the middle of a pixel (+ 0.5)
                let x_ratio = (x as f32 + 0.5) / image_width as f32;
                let y_ratio = (y as f32 + 0.5) / image_height as f32;

                // gather pixels from all images that overlap at the location in the plane
                // together with the center distance of the image to this point
                let mut overlapping_pixel = Vec::new();
                let mut distance_sum = 0.0;

                for (image, ratios, center) in image_locations.iter() {
                    if ratios.is_inside(x_ratio, y_ratio) {
                        // calculate uv for image
                        let u = (x_ratio - ratios.left) / (ratios.right - ratios.left);
                        let v = (y_ratio - ratios.top) / (ratios.bottom - ratios.top);

                        let pixel = image.get_pixel(
                            (u * image.width() as f32) as u32,
                            (v * image.height() as f32) as u32,
                        );

                        let distance_to_center = (vec2(x_ratio, y_ratio) - center).magnitude();
                        distance_sum += distance_to_center;

                        overlapping_pixel.push((pixel, distance_to_center));
                    }
                }

                // weight pixels based on distance of center
                let mut result = Rgba::from_channels(0, 0, 0, 0);

                for (pixel, distance) in overlapping_pixel.iter() {
                    // closer images get more influence
                    let weight = (distance_sum - distance) / distance_sum;

                    Self::apply_weight(&mut result, weight, **pixel);
                }

                *plane_pixel = result;
            }

            // (5) add image and buffer data
            let image =
                Image::from_raw(disparity_plane_image.into_raw(), image_width, image_height)
                    .format(VK_FORMAT_R8G8B8A8_UNORM)
                    .nearest_sampler()
                    .build(context.device(), context.queue())?;

            let current_index = image_collection.len();
            image_collection.push(image);

            vertices.extend(&LightFieldVertex::create_quad(
                left_top,
                left_bottom,
                right_top,
                right_bottom,
                current_index,
            ));
        }

        // create and setup vulkan handles
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

    #[inline]
    fn apply_weight(destination: &mut Rgba<u8>, weight: f32, source: Rgba<u8>) {
        let dst_channels = destination.channels_mut();
        let src_channels = source.channels();

        for (dst_channel, src_channel) in dst_channels.iter_mut().zip(src_channels.iter()) {
            *dst_channel += (*src_channel as f32 * weight) as u8;
        }
    }
}
