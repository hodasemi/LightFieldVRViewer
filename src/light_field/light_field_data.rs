use context::prelude::*;

use super::light_field_frustum::LightFieldFrustum;

use cgmath::{vec2, InnerSpace, Vector2, Vector3};
use image::{ImageBuffer, Rgba};

use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct LightFieldData {
    data: Vec<Plane>,
}

#[derive(Debug, Clone)]
pub struct Plane {
    pub left_top: Vector3<f32>,
    pub left_bottom: Vector3<f32>,
    pub right_top: Vector3<f32>,
    pub right_bottom: Vector3<f32>,

    // (image, corner points, center)
    pub content: Vec<(Arc<Image>, PlaneImageRatios, Vector2<f32>)>,
}

#[derive(Debug, Clone)]
pub struct PlaneImageRatios {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Debug)]
struct PlaneImage {
    image: ImageBuffer<Rgba<u8>, Vec<u8>>,
    frustum: (usize, usize),
    depth_values: Vec<f32>,
}

struct DisparityPlane {
    images: Vec<PlaneImage>,
    disparity_index: usize,
}

impl LightFieldData {
    pub fn new(
        context: &Arc<Context>,
        mut frustums: Vec<LightFieldFrustum>,
        mut image_data: Vec<(
            Vec<(ImageBuffer<Rgba<u8>, Vec<u8>>, usize, Vec<f32>)>,
            usize,
            usize,
        )>,
        frustum_extent: (usize, usize),
        baseline: f32,
    ) -> VerboseResult<LightFieldData> {
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

        let mut planes = Vec::with_capacity(disparity_planes.len());

        while let Some(mut disparity_plane) = disparity_planes.pop() {
            // calculate average depth of disparity layer
            let mut total_depth = 0.0;
            let mut total_count = 0;

            for image in disparity_plane.images.iter() {
                total_depth += image.depth_values[image.depth_values.len() / 2];
                total_count += 1;
            }

            let layer_depth = total_depth / total_count as f32;

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

                let center_x = (x + width / 2.0) / total_width;
                let center_y = (y + height / 2.0) / total_height;

                let width = image.image.width();
                let height = image.image.height();

                let vk_image = Image::from_raw(image.image.into_raw(), width, height)
                    .format(VK_FORMAT_R8G8B8A8_UNORM)
                    .nearest_sampler()
                    .build(context.device(), context.queue())?;

                image_locations.push((vk_image, ratios, vec2(center_x, center_y)));
            }

            planes.push(Plane {
                left_top,
                left_bottom,
                right_top,
                right_bottom,

                content: image_locations,
            })
        }

        Ok(LightFieldData { data: planes })
    }

    pub fn into_data(self) -> Vec<Plane> {
        self.data
    }
}
