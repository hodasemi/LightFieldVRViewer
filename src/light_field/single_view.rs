use context::prelude::*;
use image::{ImageBuffer, Pixel, Rgba};

use super::{
    super::{
        config::Config,
        example_object::ExampleVertex,
        light_field_viewer::{DEFAULT_FORWARD, UP},
    },
    LightField,
};

use cgmath::{Array, InnerSpace, Vector3};

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::thread;

const TEXTURE_WIDTH_M: f32 = 0.2;
const INTER_IMAGE_GAP_M: f32 = 0.1;

#[derive(Clone, Debug)]
struct SingleView {
    image: Arc<Image>,
    descriptor: Arc<DescriptorSet>,
    buffer: Arc<Buffer<ExampleVertex>>,
}

impl SingleView {
    fn new(
        image: Arc<Image>,
        x: u32,
        y: u32,
        config: &Config,
        plane_center: Vector3<f32>,
        right: Vector3<f32>,
        up: Vector3<f32>,
    ) -> VerboseResult<Self> {
        let w = config.extrinsics.horizontal_camera_count;
        let h = config.extrinsics.vertical_camera_count;

        // keep images ratio
        let height = (TEXTURE_WIDTH_M * image.width() as f32) / image.height() as f32;

        let complete_field_width = TEXTURE_WIDTH_M * w as f32 + INTER_IMAGE_GAP_M * (w - 1) as f32;
        let complete_field_height = height * h as f32 + INTER_IMAGE_GAP_M * (h - 1) as f32;

        let top_left_corner = plane_center - ((complete_field_width / 2.0) * right)
            + ((complete_field_height / 2.0) * up);

        let top_left = top_left_corner
            + (((TEXTURE_WIDTH_M + INTER_IMAGE_GAP_M) * x as f32) * right)
            - (((height + INTER_IMAGE_GAP_M) * y as f32) * up);
        let bottom_left = top_left_corner
            + (((TEXTURE_WIDTH_M + INTER_IMAGE_GAP_M) * x as f32) * right)
            - ((((height + INTER_IMAGE_GAP_M) * y as f32) + height) * up);

        let top_right = top_left_corner
            + ((((TEXTURE_WIDTH_M + INTER_IMAGE_GAP_M) * x as f32) + TEXTURE_WIDTH_M) * right)
            - (((height + INTER_IMAGE_GAP_M) * y as f32) * up);
        let bottom_right = top_left_corner
            + ((((TEXTURE_WIDTH_M + INTER_IMAGE_GAP_M) * x as f32) + TEXTURE_WIDTH_M) * right)
            - ((((height + INTER_IMAGE_GAP_M) * y as f32) + height) * up);

        let data = [
            ExampleVertex::pos_vec(top_left, 0.0, 0.0),
            ExampleVertex::pos_vec(bottom_left, 0.0, 1.0),
            ExampleVertex::pos_vec(bottom_right, 1.0, 1.0),
            ExampleVertex::pos_vec(bottom_right, 1.0, 1.0),
            ExampleVertex::pos_vec(top_right, 1.0, 0.0),
            ExampleVertex::pos_vec(top_left, 0.0, 0.0),
        ];

        let device = image.device();

        let buffer = Buffer::builder()
            .set_usage(VK_BUFFER_USAGE_VERTEX_BUFFER_BIT)
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_data(&data)
            .build(device.clone())?;

        let descriptor_pool = DescriptorPool::builder()
            .set_layout(LightField::descriptor_layout(device)?)
            .build(device.clone())?;

        let desc_set = DescriptorPool::prepare_set(&descriptor_pool).allocate()?;

        desc_set.update(&[DescriptorWrite::combined_samplers(0, &[&image])]);

        Ok(SingleView {
            image,
            descriptor: desc_set,
            buffer,
        })
    }

    fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        transform_descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        command_buffer.bind_descriptor_sets_minimal(&[transform_descriptor, &self.descriptor])?;
        command_buffer.bind_vertex_buffer(&self.buffer);
        command_buffer.draw_complete_single_instance(self.buffer.size() as u32);

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SingleViewLayer {
    views: Vec<SingleView>,
}

impl SingleViewLayer {
    pub fn new(
        images: Vec<(Arc<Image>, usize)>,
        x: u32,
        y: u32,
        config: &Config,
        plane_center: Vector3<f32>,
        direction: Vector3<f32>,
        right: Vector3<f32>,
        up: Vector3<f32>,
    ) -> VerboseResult<Self> {
        let image_count = images.len();
        let start = image_count / 2;

        let mut views = Vec::with_capacity(images.len());

        for (image, layer) in images.iter() {
            let direction_offset =
                ((*layer as f32 * TEXTURE_WIDTH_M) - (start as f32 * TEXTURE_WIDTH_M)) * direction;
            let single_view_plane_center = plane_center + direction_offset;

            views.push(SingleView::new(
                image.clone(),
                x,
                y,
                config,
                single_view_plane_center,
                right,
                up,
            )?);
        }

        Ok(SingleViewLayer { views })
    }

    pub fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        transform_descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        for view in self.views.iter() {
            view.render(command_buffer, transform_descriptor)?;
        }

        Ok(())
    }
}
