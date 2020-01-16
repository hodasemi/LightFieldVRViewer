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
}

#[derive(Debug, Clone)]
pub struct LightFieldFrustum {
    left: FrustumPlane,
    right: FrustumPlane,
    top: FrustumPlane,
    bottom: FrustumPlane,
    // back ?
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
        self.left.distance(point) >= 0.0
            && self.right.distance(point) >= 0.0
            && self.top.distance(point) >= 0.0
            && self.bottom.distance(point) >= 0.0
    }
}

#[derive(Debug, Clone)]
struct FrustumPlane {
    normal: Vector3<f32>,
    d: f32,
}

impl FrustumPlane {
    fn new(
        point_in_plane: Vector3<f32>,
        first_direction: Vector3<f32>,
        second_direction: Vector3<f32>,
    ) -> Self {
        let normal = first_direction.cross(second_direction).normalize();
        let d = point_in_plane.dot(normal);

        FrustumPlane { normal, d }
    }

    // https://en.wikipedia.org/wiki/Plane_%28geometry%29#Distance_from_a_point_to_a_plane
    fn distance(&self, p: Vector3<f32>) -> f32 {
        self.normal.x * p.x + self.normal.y * p.y + self.normal.z * p.z + self.d
    }
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
        mut frustums: Vec<CameraFrustum>,
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

            // (2) get layer extent
            let left_top = left_top_frustum.get_corners_at_depth(layer_depth).0;
            let left_bottom = left_bottom_frustum.get_corners_at_depth(layer_depth).1;
            let right_top = right_top_frustum.get_corners_at_depth(layer_depth).2;
            let right_bottom = right_bottom_frustum.get_corners_at_depth(layer_depth).3;

            // (3) placing images into that plane

            // since all cameras have the same aperture and baseline given in the parameters file of every light field
            // the length of every side in total = the length of the side of a camera + baseline * (cameras - 1)

            let total_width = (left_top - right_top).magnitude();

            let (frustum_width, _) = Self::frustum_extents_at_depth(left_top_frustum, layer_depth);

            let base_line_ratio = baseline / total_width;
            let width_ratio = frustum_width / total_width;

            let mut image_locations = Vec::new();

            while let Some(image) = disparity_plane.images.pop() {
                let left_ratio = base_line_ratio * image.frustum.0 as f32;
                let right_ratio = left_ratio + width_ratio;
                let top_ratio = base_line_ratio * image.frustum.1 as f32;
                let bottom_ratio = top_ratio + width_ratio;

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

        Ok(LightFieldData {
            data: planes,

            frustum,
        })
    }

    pub fn into_data(self) -> Vec<Plane> {
        self.data
    }

    fn frustum_extents_at_depth(frustum: &CameraFrustum, depth: f32) -> (f32, f32) {
        let (left_top, left_bottom, right_top, _) = frustum.get_corners_at_depth(depth);

        let width = (left_top - right_top).magnitude();
        let height = (left_top - left_bottom).magnitude();

        (width, height)
    }
}
