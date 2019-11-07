use context::prelude::*;

use super::config::Config;
use super::example_object::ExampleVertex;
use super::light_field_viewer::{DEFAULT_FORWARD, UP};

use cgmath::{Array, InnerSpace};

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::thread;

use pxm::PFM;

const TEXTURE_WIDTH_M: f32 = 0.2;
const INTER_IMAGE_GAP_M: f32 = 0.01;
const X_OFFSET_M: f32 = 0.0;
const Y_OFFSET_M: f32 = 2.0;

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
    fn new(image: Arc<Image>, x: u32, y: u32, w: u32, h: u32, z: f32) -> VerboseResult<Self> {
        // keep images ratio
        let height = (TEXTURE_WIDTH_M * image.width() as f32) / image.height() as f32;

        let complete_field_width = TEXTURE_WIDTH_M * w as f32 + INTER_IMAGE_GAP_M * (w - 1) as f32;
        let complete_field_height = height * h as f32 + INTER_IMAGE_GAP_M * (h - 1) as f32;

        let total_left = -(complete_field_width / 2.0);
        let total_top = complete_field_height / 2.0;

        let left = total_left + ((TEXTURE_WIDTH_M + INTER_IMAGE_GAP_M) * x as f32) + X_OFFSET_M;
        let right = total_left
            + ((TEXTURE_WIDTH_M + INTER_IMAGE_GAP_M) * x as f32)
            + TEXTURE_WIDTH_M
            + X_OFFSET_M;
        let top = total_top - ((height + INTER_IMAGE_GAP_M) * y as f32) + Y_OFFSET_M;
        let bottom = total_top - ((height + INTER_IMAGE_GAP_M) * y as f32) - height + Y_OFFSET_M;

        let data = [
            ExampleVertex::new(left, top, z, 0.0, 0.0),
            ExampleVertex::new(left, bottom, z, 0.0, 1.0),
            ExampleVertex::new(right, bottom, z, 1.0, 1.0),
            ExampleVertex::new(right, bottom, z, 1.0, 1.0),
            ExampleVertex::new(right, top, z, 1.0, 0.0),
            ExampleVertex::new(left, top, z, 0.0, 0.0),
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
        command_buffer.bind_descriptor_sets_minimal(&[&self.descriptor, transform_descriptor])?;
        command_buffer.bind_vertex_buffer(&self.buffer);
        command_buffer.draw_complete_single_instance(self.buffer.size() as u32);

        Ok(())
    }
}

pub struct LightField {
    pub config: Config,

    pub input_images: Vec<Vec<Option<SingleView>>>,

    _debug_image: Arc<Image>,
    debug_descriptor: Arc<DescriptorSet>,
    right_buffer: Arc<Buffer<ExampleVertex>>,
    up_buffer: Arc<Buffer<ExampleVertex>>,
}

impl LightField {
    pub fn new(context: &Arc<Context>, dir: &str) -> VerboseResult<Self> {
        let config = Config::load(&format!("{}/parameters.cfg", dir))?;

        // let depth_high_resolution = Self::load_depth_pfm(dir, "gt_depth_highres")?;
        // let depth_low_resolution = Self::load_depth_pfm(dir, "gt_depth_lowres")?;
        // let dispersion_high_resolution = Self::load_dispersion_pfm(dir, "gt_disp_highres")?;
        let dispersion_low_resolution = Self::load_dispersion_pfm(dir, "gt_disp_lowres", 10, 0.5)?;

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

        let right = direction.cross(UP).normalize();

        let plane_center = center + direction * config.extrinsics.focus_distance;
        let plane_right = plane_center + right;
        let plane_up = plane_center + UP;

        for thread in threads {
            let (image, x, y) = thread.join()??;

            input_images[x][y] = Some(SingleView::new(
                image,
                x as u32,
                y as u32,
                config.extrinsics.horizontal_camera_count,
                config.extrinsics.vertical_camera_count,
                -1.5,
            )?);
        }

        println!("finished loading light field {}", dir);

        let debug_image = Image::from_file("green.png")?
            .nearest_sampler()
            .build(context.device(), context.queue())?;

        let descriptor_pool = DescriptorPool::new()
            .set_layout(Self::descriptor_layout(context.device())?)
            .build(context.device().clone())?;

        let debug_descriptor = DescriptorPool::prepare_set(&descriptor_pool).allocate()?;

        debug_descriptor.update(&[DescriptorWrite::combined_samplers(0, &[&debug_image])]);

        let right_buffer = Buffer::new()
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_usage(VK_BUFFER_USAGE_VERTEX_BUFFER_BIT)
            .set_data(&[
                ExampleVertex::new(center.x, center.y, center.z, 0.0, 0.0),
                ExampleVertex::new(plane_center.x, plane_center.y, plane_center.z, 0.0, 1.0),
                ExampleVertex::new(plane_right.x, plane_right.y, plane_right.z, 1.0, 1.0),
            ])
            .build(context.device().clone())?;

        let up_buffer = Buffer::new()
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_usage(VK_BUFFER_USAGE_VERTEX_BUFFER_BIT)
            .set_data(&[
                ExampleVertex::new(center.x, center.y, center.z, 0.0, 0.0),
                ExampleVertex::new(plane_center.x, plane_center.y, plane_center.z, 0.0, 1.0),
                ExampleVertex::new(plane_up.x, plane_up.y, plane_up.z, 1.0, 1.0),
            ])
            .build(context.device().clone())?;

        Ok(LightField {
            config,
            input_images,

            _debug_image: debug_image,
            debug_descriptor,
            right_buffer,
            up_buffer,
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

        command_buffer
            .bind_descriptor_sets_minimal(&[&self.debug_descriptor, transform_descriptor])?;

        command_buffer.bind_vertex_buffer(&self.right_buffer);
        command_buffer.draw_complete_single_instance(self.right_buffer.size() as u32);

        command_buffer.bind_vertex_buffer(&self.up_buffer);
        command_buffer.draw_complete_single_instance(self.up_buffer.size() as u32);

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

    fn load_depth_pfm(dir: &str, file: &str) -> VerboseResult<PFM> {
        Self::open_pfm_file(&format!("{}/{}.pfm", dir, file))
    }

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
