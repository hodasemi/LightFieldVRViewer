use context::prelude::*;
use image::{ImageBuffer, Pixel, Rgba};

use super::config::Config;
use super::example_object::ExampleVertex;
use super::light_field_viewer::{DEFAULT_FORWARD, UP};

use cgmath::{Array, InnerSpace, Vector3};

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::thread;

use pxm::PFM;

const TEXTURE_WIDTH_M: f32 = 0.2;
const INTER_IMAGE_GAP_M: f32 = 0.03;
const EPSILON: f32 = 0.5;

#[derive(Clone, Debug)]
struct AlphaMap {
    data: Vec<Vec<bool>>,
}

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

        let buffer = Buffer::new()
            .set_usage(VK_BUFFER_USAGE_VERTEX_BUFFER_BIT)
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_data(&data)
            .build(device.clone())?;

        let descriptor_pool = DescriptorPool::new()
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
struct SingleViewLayer {
    views: Vec<SingleView>,
}

impl SingleViewLayer {
    fn new(
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

    fn render(
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

pub struct LightField {
    pub config: Config,

    input_images: Vec<Vec<Option<SingleViewLayer>>>,
}

impl LightField {
    pub fn new(context: &Arc<Context>, dir: &str) -> VerboseResult<Self> {
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

                    // check if image exists
                    if !Path::new(&image_path).exists() {
                        create_error!(format!("{} does not exist", image_path));
                    }

                    // check if disparity map exists
                    if !Path::new(&disparity_path).exists() {
                        create_error!(format!("{} does not exist", disparity_path));
                    }

                    let alpha_maps = Self::load_disparity_pfm(
                        &disparity_path,
                        disparity_difference as usize,
                        EPSILON,
                    )?;

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
                    let mut layer = 0;

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
                                // if *alpha_value {
                                // target_image[(x as u32, y as u32)] =
                                //     image_data[(x as u32, y as u32)];

                                // found_value = true;

                                target_image[(x as u32, y as u32)] =
                                    Rgba::from_channels(255, 0, 0, 255);
                                // }
                            }
                        }

                        // if !found_value {
                        //     layer += 1;
                        //     continue;
                        // }

                        let image = Image::from_raw(
                            target_image.into_raw(),
                            image_data.width(),
                            image_data.height(),
                        )
                        .format(VK_FORMAT_R8G8B8A8_UNORM)
                        .nearest_sampler()
                        .build(&device, &queue)?;

                        images.push((image, layer));

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

        for thread in threads {
            let (images, x, y) = thread.join()??;

            input_images[x][y] = Some(SingleViewLayer::new(
                images,
                x as u32,
                y as u32,
                &config,
                plane_center,
                direction,
                right,
                up,
            )?);
        }

        println!("finished loading light field {}", dir);

        Ok(LightField {
            config,
            input_images,
        })
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
        Ok(DescriptorSetLayout::new()
            .add_layout_binding(
                0,
                VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER,
                VK_SHADER_STAGE_FRAGMENT_BIT,
                0,
            )
            .build(device.clone())?)
    }

    // fn load_depth_pfm(dir: &str, file: &str) -> VerboseResult<PFM> {
    //     Self::open_pfm_file(&format!("{}/{}.pfm", dir, file))
    // }

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
            let x = (index as f32 / pfm.height as f32).floor() as usize;
            let y = index - (x * pfm.height);

            for (disparity, alpha_map) in alpha_maps.iter_mut().enumerate() {
                if (disp_data - disparity as f32).abs() <= epsilon {
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
