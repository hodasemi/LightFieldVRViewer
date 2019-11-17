pub mod light_field_frustum;
mod single_view;

use context::prelude::*;
use image::{ImageBuffer, Pixel, Rgba};

use super::config::Config;
use super::light_field_viewer::{DEFAULT_FORWARD, UP};

use light_field_frustum::LightFieldFrustum;
use single_view::SingleViewLayer;

use cgmath::{Array, InnerSpace, Vector3};

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::thread;

use pxm::PFM;

const EPSILON: f32 = 0.5;

#[derive(Clone, Debug)]
struct AlphaMap {
    data: Vec<Vec<bool>>,
}

pub struct LightField {
    pub config: Config,

    input_images: Vec<Vec<Option<SingleViewLayer>>>,
}

impl LightField {
    pub fn new(context: &Arc<Context>, dir: &str) -> VerboseResult<(Self, Vec<LightFieldFrustum>)> {
        let config = Config::load(&format!("{}/parameters.cfg", dir))?;

        let mut input_images = vec![
            vec![None; config.extrinsics.vertical_camera_count as usize];
            config.extrinsics.horizontal_camera_count as usize
        ];

        let mut threads = Vec::with_capacity(
            (config.extrinsics.horizontal_camera_count * config.extrinsics.vertical_camera_count)
                as usize,
        );

        let mut total_index = 0;
        let disparity_difference = config.meta.disp_min.abs() + config.meta.disp_max.abs();

        for (x, row) in input_images.iter().enumerate() {
            for (y, _image) in row.iter().enumerate() {
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

                    let alpha_maps = Self::load_disparity_pfm(
                        &disparity_path,
                        disparity_difference as usize,
                        EPSILON,
                    )?;

                    let depth_pfm = Self::open_pfm_file(&depth_path)?;

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
                    let mut layer: usize = 0;

                    for alpha_map in alpha_maps {
                        let mut target_image: ImageBuffer<Rgba<u8>, Vec<u8>> =
                            ImageBuffer::from_pixel(
                                image_data.width(),
                                image_data.height(),
                                Rgba::from_channels(0, 0, 0, 0),
                            );

                        let mut found_value = false;

                        for (x, row) in alpha_map.data.iter().enumerate() {
                            for (y, alpha_value) in row.iter().enumerate() {
                                if *alpha_value {
                                    target_image[(x as u32, y as u32)] =
                                        image_data[(x as u32, y as u32)];

                                    found_value = true;
                                } else {
                                    target_image[(x as u32, y as u32)] =
                                        Rgba::from_channels(255, 0, 0, 255);

                                    // found_value = true;
                                }
                            }
                        }

                        if found_value {
                            let image = Image::from_raw(
                                target_image.into_raw(),
                                image_data.width(),
                                image_data.height(),
                            )
                            .format(VK_FORMAT_R8G8B8A8_UNORM)
                            .nearest_sampler()
                            .build(&device, &queue)?;

                            images.push((image, layer));
                        }

                        layer += 1;
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

        let plane_center = center + direction * config.extrinsics.focus_distance;

        let frustums =
            LightFieldFrustum::create_frustums(plane_center, direction, up, right, &config);

        let mut images = Vec::with_capacity(threads.len());

        for thread in threads {
            images.push(thread.join()??);

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

        println!("finished loading light field {}", dir);

        Ok((
            LightField {
                config,
                input_images,
            },
            frustums,
        ))
    }

    pub fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        transform_descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        for row in self.input_images.iter() {
            for single_view_opt in row.iter() {
                if let Some(single_view) = single_view_opt {
                    single_view.render(command_buffer, transform_descriptor)?;
                }
            }
        }

        Ok(())
    }

    pub fn descriptor_layout(device: &Arc<Device>) -> VerboseResult<Arc<DescriptorSetLayout>> {
        Ok(DescriptorSetLayout::builder()
            .add_layout_binding(
                0,
                VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER,
                VK_SHADER_STAGE_FRAGMENT_BIT,
                0,
            )
            .build(device.clone())?)
    }

    fn load_disparity_pfm(
        path: &str,
        alpha_map_count: usize,
        epsilon: f32,
    ) -> VerboseResult<Vec<AlphaMap>> {
        let pfm = Self::open_pfm_file(path)?;

        let mut alpha_maps = vec![
            AlphaMap {
                data: vec![vec![false; pfm.height as usize]; pfm.width as usize],
            };
            alpha_map_count
        ];

        for (index, disp_data) in pfm.data.iter().enumerate() {
            let y = (index as f32 / pfm.height as f32).floor() as usize;
            let x = index - (y * pfm.height);

            for (disparity, alpha_map) in alpha_maps.iter_mut().enumerate() {
                if (disp_data.abs() - disparity as f32).abs() <= epsilon {
                    alpha_map.data[x][y] = true;
                }
            }
        }

        Ok(alpha_maps)
    }

    #[inline]
    fn open_pfm_file(path: &str) -> VerboseResult<PFM> {
        let pfm_file = File::open(path)?;
        let mut pfm_bufreader = BufReader::new(pfm_file);

        Ok(PFM::read_from(&mut pfm_bufreader)?)
    }

    #[inline]
    fn swap_axis(mut v: Vector3<f32>) -> Vector3<f32> {
        v.swap_elements(1, 2);
        v.z = -v.z;

        v
    }
}
