mod alpha_maps;
pub mod light_field_data;
pub mod light_field_frustum;

use ordered_float::OrderedFloat;

use context::prelude::*;
use image::{ImageBuffer, Pixel, Rgba};

use super::config::Config;
use super::light_field_viewer::{DEFAULT_FORWARD, UP};

use alpha_maps::AlphaMaps;
use light_field_data::{LightFieldData, LightFieldFrustum, Plane};
use light_field_frustum::CameraFrustum;

use cgmath::{Array, InnerSpace, Vector3};

use pxm::PFM;

use std::path::Path;
use std::sync::Arc;
use std::thread;

struct DepthInfo {
    depth_maps: Vec<PFM>,

    indices: Vec<usize>,

    min: f32,
    max: f32,
}

pub struct LightField {
    pub config: Config,

    pub center: Vector3<f32>,
    pub direction: Vector3<f32>,
    pub up: Vector3<f32>,
    pub right: Vector3<f32>,

    light_field_data: LightFieldData,
}

impl LightField {
    pub fn new(context: &Arc<Context>, dir: &str, number_of_slices: usize) -> VerboseResult<Self> {
        println!("started loading light field {}", dir);

        let config = Config::load(&format!("{}/parameters.cfg", dir))?;

        let mut threads = Vec::with_capacity(
            (config.extrinsics.horizontal_camera_count * config.extrinsics.vertical_camera_count)
                as usize,
        );

        let depth_info = Self::load_depth_maps(
            dir,
            config.extrinsics.horizontal_camera_count as usize
                * config.extrinsics.vertical_camera_count as usize,
        )?;

        let slice_thickness = (depth_info.max - depth_info.min) / number_of_slices as f32;
        let minimum_depth = depth_info.min;

        for (i, (pfm, index)) in depth_info
            .depth_maps
            .into_iter()
            .zip(depth_info.indices.iter())
            .enumerate()
        {
            assert_eq!(i, *index);

            let (x, y) = AlphaMaps::to_xy(config.extrinsics.vertical_camera_count as usize, i);

            let meta_image_width = config.intrinsics.image_width;
            let meta_image_height = config.intrinsics.image_height;

            let dir = dir.to_string();

            threads.push(thread::spawn(move || {
                let image_path = format!("{}/input_Cam{:03}.png", dir, i);

                // check if image exists
                if !Path::new(&image_path).exists() {
                    create_error!(format!("{} does not exist", image_path));
                }

                let alpha_maps =
                    AlphaMaps::new(pfm, number_of_slices, minimum_depth, slice_thickness)?;

                let image_data = match image::open(&image_path) {
                    Ok(tex) => tex.to_rgba(),
                    Err(err) => {
                        create_error!(format!("error loading image (\"{}\"): {}", image_path, err))
                    }
                };

                // check if texture dimensions match meta information
                if image_data.width() != meta_image_width
                    || image_data.height() != meta_image_height
                {
                    create_error!(format!(
                        "Image ({}) has a not expected extent, expected: {:?} found: {:?}",
                        image_path,
                        (meta_image_width, meta_image_height),
                        (image_data.width(), image_data.height())
                    ));
                }

                // let mut images = Vec::with_capacity(alpha_maps.len());
                let mut images = Vec::new();

                for (layer_index, alpha_map) in alpha_maps.iter().enumerate() {
                    let mut target_image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(
                        image_data.width(),
                        image_data.height(),
                        Rgba::from_channels(0, 0, 0, 0),
                    );

                    let mut found_value = false;

                    alpha_map.for_each_alpha(|x, y| {
                        target_image[(x as u32, y as u32)] = image_data[(x as u32, y as u32)];
                        found_value = true;
                    });

                    if found_value {
                        images.push((target_image, layer_index, alpha_map.depth_values().clone()));
                    }
                }

                Ok((images, x, y))
            }));
        }

        // swap y and z coordinates
        let center = Self::swap_axis(config.extrinsics.camera_center);

        let direction = Self::swap_axis(
            (config.extrinsics.camera_rotation_matrix() * DEFAULT_FORWARD.extend(1.0))
                .truncate()
                .normalize(),
        );

        let up = Self::swap_axis(
            (config.extrinsics.camera_rotation_matrix() * UP.extend(1.0))
                .truncate()
                .normalize(),
        );

        let right = direction.cross(up).normalize();

        let frustums = CameraFrustum::create_frustums(center, direction, up, right, &config);

        let mut image_data = Vec::with_capacity(threads.len());

        for thread in threads {
            image_data.push(thread.join()??);
        }

        let light_field_data = LightFieldData::new(
            context,
            frustums,
            image_data,
            (
                config.extrinsics.horizontal_camera_count as usize,
                config.extrinsics.vertical_camera_count as usize,
            ),
            config.extrinsics.baseline(),
            depth_info.max,
        )?;

        println!("finished loading light field {}", dir);

        Ok(LightField {
            config,

            center,
            direction,
            up,
            right,

            light_field_data,
        })
    }

    fn load_depth_maps(dir: &str, max_maps: usize) -> VerboseResult<DepthInfo> {
        let mut depth_maps = Vec::new();
        let mut min = OrderedFloat(std::f32::MAX);
        let mut max = OrderedFloat(std::f32::MIN);
        let mut indices = Vec::new();

        for i in 0..max_maps {
            let depth_path = format!("{}/gt_depth_lowres_Cam{:03}.pfm", dir, i);

            // check if depth map exists
            if !Path::new(&depth_path).exists() {
                create_error!(format!("{} does not exist", depth_path));
            }

            let depth_pfm = AlphaMaps::open_pfm_file(&depth_path)?;

            for depth in depth_pfm.data.iter() {
                if *depth < 10000.0 {
                    min = std::cmp::min(min, OrderedFloat(*depth));
                    max = std::cmp::max(max, OrderedFloat(*depth));
                }
            }

            depth_maps.push(depth_pfm);
            indices.push(i);
        }

        Ok(DepthInfo {
            depth_maps,

            indices,

            min: min.0,
            max: max.0,
        })
    }

    pub fn frustum(&self) -> LightFieldFrustum {
        self.light_field_data.frustum.clone()
    }

    pub fn outlines(&self) -> [(Vector3<f32>, Vector3<f32>); 4] {
        self.light_field_data.frustum_edges
    }

    pub fn direction(&self) -> Vector3<f32> {
        self.light_field_data.direction
    }

    pub fn into_data(self) -> Vec<Plane> {
        self.light_field_data.into_data()
    }

    pub fn is_empty(&self) -> bool {
        self.light_field_data.is_empty()
    }

    #[inline]
    fn swap_axis(mut v: Vector3<f32>) -> Vector3<f32> {
        v.swap_elements(1, 2);
        v.z = -v.z;

        v
    }
}
