use context::prelude::*;

use super::config::Config;
use super::example_object::ExampleVertex;

use std::path::Path;
use std::sync::Arc;
use std::thread;

#[derive(Clone, Debug)]
pub struct SingleView {
    image: Arc<Image>,
    descriptor: Arc<DescriptorSet>,
    buffer: Arc<Buffer<ExampleVertex>>,
}

impl SingleView {
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
}

impl LightField {
    pub fn new(context: &Arc<Context>, dir: &str) -> VerboseResult<Self> {
        let config = Config::load(&format!("{}/parameters.cfg", dir))?;

        let mut input_images = vec![
            vec![None; config.extrinsics.horizontal_camera_count as usize];
            config.extrinsics.vertical_camera_count as usize
        ];

        let mut threads = Vec::with_capacity(
            (config.extrinsics.horizontal_camera_count * config.extrinsics.vertical_camera_count)
                as usize,
        );

        let mut total_index = 0;

        for (y, col) in input_images.iter().enumerate() {
            for (x, _image) in col.iter().enumerate() {
                let queue = context.queue().clone();
                let device = context.device().clone();

                let meta_image_width = config.intrinsics.image_width;
                let meta_image_height = config.intrinsics.image_height;

                let dir = dir.to_string();

                threads.push(thread::spawn(move || {
                    let path = format!("{}/input_Cam{:03}.png", dir, total_index);

                    let image = if Path::new(&path).exists() {
                        println!("loading image {}", path);

                        let image = Image::from_file(&path)?
                            .nearest_sampler()
                            .build(&device, &queue)?;

                        println!("loading finished ({})", path);

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

        for thread in threads {
            if let Ok(thread_result) = thread.join() {
                if let Ok((image, x, y)) = thread_result {
                    input_images[x][y] = Some(Self::create_single_view(
                        image,
                        x as u32,
                        y as u32,
                        config.extrinsics.horizontal_camera_count,
                        config.extrinsics.vertical_camera_count,
                        -3.0,
                    )?);
                }
            }
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
        DescriptorSetLayout::new()
            .add_layout_binding(
                0,
                VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER,
                VK_SHADER_STAGE_FRAGMENT_BIT,
                0,
            )
            .build(device.clone())
    }

    fn create_single_view(
        image: Arc<Image>,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        z: f32,
    ) -> VerboseResult<SingleView> {
        // width of texture = 20 centimeters
        let width = 0.2;
        // keep images ratio
        let height = (width * image.width() as f32) / image.height() as f32;

        // gap between images = 5 centimeters
        let inter_image_gap = 0.05;

        let complete_field_width = width * w as f32 + inter_image_gap * (w - 1) as f32;
        let complete_field_height = height * h as f32 + inter_image_gap * (h - 1) as f32;

        let total_left = -(complete_field_width / 2.0);
        let total_top = complete_field_height / 2.0;

        let left = total_left + ((width + inter_image_gap) * x as f32);
        let right = total_left + ((width + inter_image_gap) * x as f32) + width;
        let top = total_top - ((height + inter_image_gap) * y as f32);
        let bottom = total_top - ((height + inter_image_gap) * y as f32) - height;

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
            .set_layout(Self::descriptor_layout(device)?)
            .build(device.clone())?;

        let desc_set = DescriptorPool::prepare_set(&descriptor_pool).allocate()?;

        desc_set.update(&[DescriptorWrite::combined_samplers(0, &[&image])]);

        Ok(SingleView {
            image,
            descriptor: desc_set,
            buffer,
        })
    }
}