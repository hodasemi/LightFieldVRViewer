mod alpha_maps;
pub mod light_field_frustum;
pub mod light_field_renderer;

use context::prelude::*;
use image::{ImageBuffer, Pixel, Rgba};

use super::config::Config;
use super::light_field_viewer::{DEFAULT_FORWARD, UP};

use alpha_maps::AlphaMaps;
use light_field_frustum::LightFieldFrustum;
use light_field_renderer::LightFieldRenderer;

use cgmath::{Array, InnerSpace, Vector3};

use std::path::Path;
use std::sync::Arc;
use std::thread;

const EPSILON: f32 = 0.5;

pub struct LightField {
    pub config: Config,

    light_field_renderer: LightFieldRenderer,
}

impl LightField {
    pub fn new(context: &Arc<Context>, dir: &str) -> VerboseResult<(Self, Vec<LightFieldFrustum>)> {
        let config = Config::load(&format!("{}/parameters.cfg", dir))?;

        // let mut input_images = vec![
        //     vec![None; config.extrinsics.vertical_camera_count as usize];
        //     config.extrinsics.horizontal_camera_count as usize
        // ];

        let mut threads = Vec::with_capacity(
            (config.extrinsics.horizontal_camera_count * config.extrinsics.vertical_camera_count)
                as usize,
        );

        let mut total_index = 0;
        let disparity_difference = config.meta.disp_min.abs() + config.meta.disp_max.abs();

        for x in 0..config.extrinsics.horizontal_camera_count as usize {
            for y in 0..config.extrinsics.vertical_camera_count as usize {
                let queue = context.queue().clone();
                let device = context.device().clone();

                let meta_image_width = config.intrinsics.image_width;
                let meta_image_height = config.intrinsics.image_height;

                let dir = dir.to_string();

                threads.push(thread::spawn(move || {
                    let image_path = format!("{}/input_Cam{:03}.png", dir, total_index);
                    let disparity_path =
                        format!("{}/gt_disp_lowres_Cam{:03}.pfm", dir, total_index);
                    let depth_path = format!("{}/gt_depth_lowres_Cam{:03}.pfm", dir, total_index);

                    // check if image exists
                    if !Path::new(&image_path).exists() {
                        create_error!(format!("{} does not exist", image_path));
                    }

                    // check if disparity map exists
                    if !Path::new(&disparity_path).exists() {
                        create_error!(format!("{} does not exist", disparity_path));
                    }

                    // check if depth map exists
                    if !Path::new(&depth_path).exists() {
                        create_error!(format!("{} does not exist", depth_path));
                    }

                    let alpha_maps =
                        AlphaMaps::new(&disparity_path, disparity_difference as usize, EPSILON)?
                            .load_depth(&depth_path)?;

                    let image_data = match image::open(&image_path) {
                        Ok(tex) => tex.to_rgba(),
                        Err(err) => create_error!(format!(
                            "error loading image (\"{}\"): {}",
                            image_path, err
                        )),
                    };

                    // check if texture dimensions match meta information
                    if image_data.width() != meta_image_width
                        || image_data.height() != meta_image_height
                    {
                        create_error!(format!("Image ({}) has a not expected extent", image_path));
                    }

                    let mut images = Vec::with_capacity(alpha_maps.len());

                    for alpha_map in alpha_maps.iter() {
                        let mut target_image: ImageBuffer<Rgba<u8>, Vec<u8>> =
                            ImageBuffer::from_pixel(
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
                            let image = Image::from_raw(
                                target_image.into_raw(),
                                image_data.width(),
                                image_data.height(),
                            )
                            .format(VK_FORMAT_R8G8B8A8_UNORM)
                            .nearest_sampler()
                            .build(&device, &queue)?;

                            images.push((
                                image,
                                alpha_map
                                    .depth()
                                    .ok_or("no depth attached to this alpha map")?,
                            ));
                        }
                    }

                    Ok((images, x, y))
                }));

                total_index += 1;
            }
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

        // let plane_center = center + direction * config.extrinsics.focus_distance;

        let frustums = LightFieldFrustum::create_frustums(center, direction, up, right, &config);

        let mut image_data = Vec::with_capacity(threads.len());

        for thread in threads {
            image_data.push(thread.join()??);

            // input_images[x][y] = Some(SingleViewLayer::new(
            //     images,
            //     x as u32,
            //     y as u32,
            //     &config,
            //     plane_center,
            //     direction,
            //     right,
            //     up,
            // )?);
        }

        let light_field_renderer = LightFieldRenderer::new(context, frustums.clone(), image_data)?;

        println!("finished loading light field {}", dir);

        Ok((
            LightField {
                config,
                light_field_renderer,
            },
            frustums,
        ))
    }

    pub fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        transform_descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        // for row in self.input_images.iter() {
        //     for single_view_opt in row.iter() {
        //         if let Some(single_view) = single_view_opt {
        //             single_view.render(command_buffer, transform_descriptor)?;
        //         }
        //     }
        // }

        self.light_field_renderer
            .render(command_buffer, transform_descriptor)?;

        Ok(())
    }

    #[inline]
    fn swap_axis(mut v: Vector3<f32>) -> Vector3<f32> {
        v.swap_elements(1, 2);
        v.z = -v.z;

        v
    }
}
