use context::prelude::*;

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
const INTER_IMAGE_GAP_M: f32 = 0.01;

#[derive(Clone, Debug)]
pub struct AlphaMap {
    data: Vec<Vec<bool>>,
}

#[derive(Clone, Debug)]
pub struct SingleView {
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

pub struct LightField {
    pub config: Config,

    pub input_images: Vec<Vec<Option<SingleView>>>,
}

impl LightField {
    pub fn new(context: &Arc<Context>, dir: &str) -> VerboseResult<Self> {
        let config = Config::load(&format!("{}/parameters.cfg", dir))?;

        // let depth_high_resolution = Self::load_depth_pfm(dir, "gt_depth_highres")?;
        // let depth_low_resolution = Self::load_depth_pfm(dir, "gt_depth_lowres")?;
        // let dispersion_high_resolution = Self::load_dispersion_pfm(dir, "gt_disp_highres")?;
        let dispersion_difference = config.meta.disp_min.abs() + config.meta.disp_max.abs();

        let _dispersion_low_resolution =
            Self::load_dispersion_pfm(dir, "gt_disp_lowres", dispersion_difference as usize, 0.5)?;

        let mut input_images = vec![
            vec![None; config.extrinsics.vertical_camera_count as usize];
            config.extrinsics.horizontal_camera_count as usize
        ];

        let mut threads = Vec::with_capacity(
            (config.extrinsics.horizontal_camera_count * config.extrinsics.vertical_camera_count)
                as usize,
        );

        let mut total_index = 0;

        for (x, row) in input_images.iter().enumerate() {
            for (y, _image) in row.iter().enumerate() {
                let queue = context.queue().clone();
                let device = context.device().clone();

                let meta_image_width = config.intrinsics.image_width;
                let meta_image_height = config.intrinsics.image_height;

                let dir = dir.to_string();

                threads.push(thread::spawn(move || {
                    let path = format!("{}/input_Cam{:03}.png", dir, total_index);

                    let image = if Path::new(&path).exists() {
                        let image = Image::from_file(&path)?
                            .nearest_sampler()
                            .build(&device, &queue)?;

                        // check if texture dimensions match meta information
                        if image.width() != meta_image_width || image.height() != meta_image_height
                        {
                            create_error!(format!("Image ({}) has a not expected extent", path));
                        }

                        image
                    } else {
                        create_error!(format!("{} does not exist", path));
                    };

                    Ok((image, x, y))
                }));

                total_index += 1;
            }
        }

        // swap y and z coordinates
        let mut center = config.extrinsics.camera_center;
        center.swap_elements(1, 2);

        let mut direction = (config.extrinsics.camera_rotation_matrix()
            * DEFAULT_FORWARD.extend(1.0))
        .truncate()
        .normalize();

        direction.swap_elements(1, 2);

        let mut up = (config.extrinsics.camera_rotation_matrix() * UP.extend(1.0))
            .truncate()
            .normalize();

        up.swap_elements(1, 2);

        let right = direction.cross(up).normalize();

        let plane_center = center + direction * config.extrinsics.focus_distance;

        for thread in threads {
            let (image, x, y) = thread.join()??;

            input_images[x][y] = Some(SingleView::new(
                image,
                x as u32,
                y as u32,
                &config,
                plane_center,
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

    fn load_dispersion_pfm(
        dir: &str,
        file: &str,
        alpha_map_count: usize,
        epsilon: f32,
    ) -> VerboseResult<Vec<AlphaMap>> {
        let pfm = Self::open_pfm_file(&format!("{}/{}.pfm", dir, file))?;

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
}
