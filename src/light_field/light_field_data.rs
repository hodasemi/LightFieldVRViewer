use context::prelude::*;

use super::light_field_frustum::CameraFrustum;

use cgmath::{vec2, InnerSpace, Vector2, Vector3};
use image::{ImageBuffer, Rgba};

use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct LightFieldData {
    data: Vec<Plane>,

    pub frustum: LightFieldFrustum,
    pub direction: Vector3<f32>,

    // (start, end) of 4 lines
    pub frustum_edges: [(Vector3<f32>, Vector3<f32>); 4],
}

#[derive(Debug, Clone)]
pub struct LightFieldFrustum {
    left: FrustumPlane,
    right: FrustumPlane,
    top: FrustumPlane,
    bottom: FrustumPlane,
    // front + back ?
}

impl LightFieldFrustum {
    fn new(
        left_top_frustum: &CameraFrustum,
        right_top_frustum: &CameraFrustum,
        left_bottom_frustum: &CameraFrustum,
        right_bottom_frustum: &CameraFrustum,
    ) -> Self {
        let left_top = &left_top_frustum.left_top;
        let right_top = &right_top_frustum.right_top;
        let left_bottom = &left_bottom_frustum.left_bottom;
        let right_bottom = &right_bottom_frustum.right_bottom;

        LightFieldFrustum {
            left: FrustumPlane::new(left_top.center, left_top.direction, left_bottom.direction),
            right: FrustumPlane::new(
                right_top.center,
                right_bottom.direction,
                right_top.direction,
            ),
            top: FrustumPlane::new(left_top.center, right_top.direction, left_top.direction),
            bottom: FrustumPlane::new(
                left_bottom.center,
                left_bottom.direction,
                right_bottom.direction,
            ),
        }
    }

    pub fn check(&self, point: Vector3<f32>) -> bool {
        self.left.is_above(point)
            && self.right.is_above(point)
            && self.top.is_above(point)
            && self.bottom.is_above(point)
    }
}

#[derive(Debug, Clone)]
pub struct FrustumPlane {
    pub point: Vector3<f32>,
    pub normal: Vector3<f32>,
}

impl FrustumPlane {
    pub fn new(
        point_in_plane: Vector3<f32>,
        first_direction: Vector3<f32>,
        second_direction: Vector3<f32>,
    ) -> Self {
        let normal = first_direction.cross(second_direction).normalize();

        FrustumPlane {
            point: point_in_plane,
            normal,
        }
    }

    pub fn is_above(&self, p: Vector3<f32>) -> bool {
        self.normal.dot(self.point - p) > 0.0
    }
}

#[derive(Debug, Clone)]
pub struct Plane {
    pub left_top: Vector3<f32>,
    pub left_bottom: Vector3<f32>,
    pub right_top: Vector3<f32>,
    pub right_bottom: Vector3<f32>,

    // (image, extents, center)
    pub content: Vec<(Arc<Image>, PlaneImageRatios, Vector2<f32>)>,
}

#[derive(Debug, Clone, Default, Copy)]
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

struct QuantizedPlane {
    images: Vec<PlaneImage>,
    layer_index: usize,
}

impl LightFieldData {
    pub fn new(
        context: &Arc<Context>,
        frustums: Vec<CameraFrustum>,
        image_data: Vec<(
            Vec<(ImageBuffer<Rgba<u8>, Vec<u8>>, usize, Vec<f32>)>,
            usize,
            usize,
        )>,
        frustum_extent: (usize, usize),
        baseline: f32,
        max_depth: f32,
    ) -> VerboseResult<LightFieldData> {
        // create a map for frustums
        let mut sorted_frustums = HashMap::new();

        for frustum in frustums.into_iter() {
            sorted_frustums.insert(frustum.position(), frustum);
        }

        // sort all images by their respective layer
        let mut quantized_planes: Vec<QuantizedPlane> = Vec::new();

        for (images, x, y) in image_data.into_iter() {
            for (image, layer_index, depth_values) in images.into_iter() {
                // create plane image
                let plane_image = PlaneImage {
                    image,
                    frustum: (x, y),
                    depth_values,
                };

                // search for layer index
                match quantized_planes
                    .iter()
                    .position(|plane| plane.layer_index == layer_index)
                {
                    // if we can find the layer, just add the plane image
                    Some(index) => quantized_planes[index].images.push(plane_image),

                    // if we couldn't find the layer, push layer and image
                    None => quantized_planes.push(QuantizedPlane {
                        images: vec![plane_image],
                        layer_index,
                    }),
                }
            }
        }

        let mut planes = Vec::with_capacity(quantized_planes.len());

        // (1) find corner frustums
        let left_top_frustum = &sorted_frustums[&(0, 0)];
        let left_bottom_frustum = &sorted_frustums[&(0, frustum_extent.1 - 1)];
        let right_top_frustum = &sorted_frustums[&(frustum_extent.0 - 1, 0)];
        let right_bottom_frustum = &sorted_frustums[&(frustum_extent.0 - 1, frustum_extent.1 - 1)];

        let frustum = LightFieldFrustum::new(
            left_top_frustum,
            right_top_frustum,
            left_bottom_frustum,
            right_bottom_frustum,
        );

        for quantized_plane in quantized_planes.into_iter() {
            // calculate average depth of the layer
            let mut total_depth = 0.0;
            let mut total_count = 0;

            for image in quantized_plane.images.iter() {
                total_depth += image.depth_values[image.depth_values.len() / 2];
                total_count += 1;
            }

            let layer_depth = total_depth / total_count as f32;

            if layer_depth > 100000.0 {
                continue;
            }

            // (2) get layer extent
            let left_top = left_top_frustum.get_corners_at_depth(layer_depth).0;
            let left_bottom = left_bottom_frustum.get_corners_at_depth(layer_depth).1;
            let right_top = right_top_frustum.get_corners_at_depth(layer_depth).2;
            let right_bottom = right_bottom_frustum.get_corners_at_depth(layer_depth).3;

            // (3) placing images into that plane

            // since all cameras have the same aperture and baseline given in the parameters file of every light field
            // the length of every side in total = the length of the side of a camera + baseline * (cameras - 1)

            let total_width = (left_top - right_top).magnitude();
            let total_height = (left_top - left_bottom).magnitude();

            let (frustum_width, frustum_height) =
                Self::frustum_extents_at_depth(left_top_frustum, layer_depth);

            let horizontal_base_line_ratio = baseline / total_width;
            let vertical_base_line_ratio = baseline / total_height;
            let width_ratio = frustum_width / total_width;
            let height_ration = frustum_height / total_height;

            let mut image_locations = Vec::new();

            for image in quantized_plane.images.into_iter() {
                let left_ratio = horizontal_base_line_ratio * image.frustum.1 as f32;
                let right_ratio = left_ratio + width_ratio;
                let top_ratio = vertical_base_line_ratio * image.frustum.0 as f32;
                let bottom_ratio = top_ratio + height_ration;

                let ratios = PlaneImageRatios {
                    left: left_ratio,
                    right: right_ratio,
                    top: top_ratio,
                    bottom: bottom_ratio,
                };

                let center_x = (left_ratio + right_ratio) / 2.0;
                let center_y = (top_ratio + bottom_ratio) / 2.0;

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

        let frustum_edges = [
            (
                left_top_frustum.get_corners_at_depth(0.0).0,
                left_top_frustum.get_corners_at_depth(max_depth).0,
            ),
            (
                left_bottom_frustum.get_corners_at_depth(0.0).1,
                left_bottom_frustum.get_corners_at_depth(max_depth).1,
            ),
            (
                right_top_frustum.get_corners_at_depth(0.0).2,
                right_top_frustum.get_corners_at_depth(max_depth).2,
            ),
            (
                right_bottom_frustum.get_corners_at_depth(0.0).3,
                right_bottom_frustum.get_corners_at_depth(max_depth).3,
            ),
        ];

        Ok(LightFieldData {
            data: planes,

            frustum,

            direction: left_top_frustum.main_direction,

            frustum_edges,
        })
    }

    pub fn into_data(self) -> Vec<Plane> {
        self.data
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn frustum_extents_at_depth(frustum: &CameraFrustum, depth: f32) -> (f32, f32) {
        let (left_top, left_bottom, right_top, _) = frustum.get_corners_at_depth(depth);

        let width = (left_top - right_top).magnitude();
        let height = (left_top - left_bottom).magnitude();

        (width, height)
    }
}
