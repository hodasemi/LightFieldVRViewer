use context::prelude::*;

use cgmath::Vector4;

use crate::{
    interpolation::{LightField, Plane},
    light_field::light_field_data::PlaneImageRatios,
};

use std::sync::{
    atomic::{AtomicUsize, Ordering::SeqCst},
    Arc, Mutex,
};

pub struct LayerDebugger {
    as_descriptor: Arc<DescriptorSet>,

    pipeline: Arc<Pipeline>,
    sbt: ShaderBindingTable,

    layer_index: AtomicUsize,

    plane_buffer: Arc<Buffer<DebugPlane>>,
    image_info_buffer: Mutex<Arc<Buffer<DebugImageInfo>>>,
    tlas: Mutex<Option<Arc<AccelerationStructure>>>,
    specific_image: AtomicUsize,
}

impl LayerDebugger {
    pub fn new(
        context: &Arc<Context>,
        images: &Vec<Arc<Image>>,
        view_descriptor_layer: &dyn VkHandle<VkDescriptorSetLayout>,
        output_image_descriptor_layout: &dyn VkHandle<VkDescriptorSetLayout>,
    ) -> VerboseResult<Self> {
        let device = context.device();

        let as_desc = Self::create_as_descriptor(device, images)?;

        let pipeline_layout = PipelineLayout::builder()
            .add_descriptor_set_layout(&as_desc)
            .add_descriptor_set_layout(view_descriptor_layer)
            .add_descriptor_set_layout(output_image_descriptor_layout)
            .build(device.clone())?;

        let (pipeline, sbt) = Pipeline::new_ray_tracing()
            .add_shader(
                ShaderModule::from_slice(
                    device.clone(),
                    include_bytes!("../../shader/debug/raygen.rgen.spv"),
                    ShaderType::RayGeneration,
                )?,
                None,
                None,
            )
            .add_shader(
                ShaderModule::from_slice(
                    device.clone(),
                    include_bytes!("../../shader/debug/miss.rmiss.spv"),
                    ShaderType::Miss,
                )?,
                None,
                None,
            )
            .add_hit_shaders(
                vec![ShaderModule::from_slice(
                    device.clone(),
                    include_bytes!("../../shader/debug/closesthit.rchit.spv"),
                    ShaderType::ClosestHit,
                )?],
                None,
                vec![None],
            )
            .build(device, &pipeline_layout)?;

        let plane_buffer = Buffer::builder()
            .set_memory_properties(
                VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT,
            )
            .set_usage(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT)
            .set_size(1)
            .build(device.clone())?;

        let image_info_buffer = Buffer::builder()
            .set_memory_properties(
                VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT,
            )
            .set_usage(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT)
            .set_size(1)
            .build(device.clone())?;

        Ok(LayerDebugger {
            as_descriptor: as_desc,

            pipeline,
            sbt,

            layer_index: AtomicUsize::new(0),

            plane_buffer,
            image_info_buffer: Mutex::new(image_info_buffer),
            tlas: Mutex::new(None),
            specific_image: AtomicUsize::new(9),
        })
    }

    fn create_as_descriptor(
        device: &Arc<Device>,
        images: &Vec<Arc<Image>>,
    ) -> VerboseResult<Arc<DescriptorSet>> {
        let descriptor_set_layout = DescriptorSetLayout::builder()
            .add_layout_binding(
                0,
                VK_DESCRIPTOR_TYPE_ACCELERATION_STRUCTURE_NV,
                VK_SHADER_STAGE_RAYGEN_BIT_NV,
                0,
            )
            .add_layout_binding(
                1,
                VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
                VK_SHADER_STAGE_CLOSEST_HIT_BIT_NV,
                0,
            )
            .add_layout_binding(
                2,
                VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
                VK_SHADER_STAGE_CLOSEST_HIT_BIT_NV,
                0,
            )
            .add_layout_binding(
                3,
                VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER,
                VK_SHADER_STAGE_CLOSEST_HIT_BIT_NV,
                VK_DESCRIPTOR_BINDING_VARIABLE_DESCRIPTOR_COUNT_BIT_EXT,
            )
            .change_descriptor_count(images.len() as u32)
            .build(device.clone())?;

        let descriptor_pool = DescriptorPool::builder()
            .set_layout(descriptor_set_layout)
            .build(device.clone())?;

        let desc_set = DescriptorPool::prepare_set(&descriptor_pool).allocate()?;

        let image_refs: Vec<&Arc<Image>> = images.iter().map(|image| image).collect();

        desc_set.update(&[DescriptorWrite::combined_samplers(3, &image_refs)]);

        Ok(desc_set)
    }

    pub fn handle_input(&self, key_code: Keycode) {
        match key_code {
            Keycode::Plus => {
                self.layer_index.fetch_add(1, SeqCst);

                println!("debugging {}. layer", self.layer_index.load(SeqCst));
            }
            Keycode::Minus => {
                let index = self.layer_index.load(SeqCst);

                if index > 0 {
                    self.layer_index.fetch_sub(1, SeqCst);
                }

                println!("debugging {}. layer", self.layer_index.load(SeqCst));
            }
            Keycode::Num1 => {
                self.specific_image.store(0, SeqCst);
                println!("specific image: {}", self.specific_image.load(SeqCst))
            }
            Keycode::Num2 => {
                self.specific_image.store(1, SeqCst);
                println!("specific image: {}", self.specific_image.load(SeqCst))
            }
            Keycode::Num3 => {
                self.specific_image.store(2, SeqCst);
                println!("specific image: {}", self.specific_image.load(SeqCst))
            }
            Keycode::Num4 => {
                self.specific_image.store(3, SeqCst);
                println!("specific image: {}", self.specific_image.load(SeqCst))
            }
            Keycode::Num5 => {
                self.specific_image.store(4, SeqCst);
                println!("specific image: {}", self.specific_image.load(SeqCst))
            }
            Keycode::Num6 => {
                self.specific_image.store(5, SeqCst);
                println!("specific image: {}", self.specific_image.load(SeqCst))
            }
            Keycode::Num7 => {
                self.specific_image.store(6, SeqCst);
                println!("specific image: {}", self.specific_image.load(SeqCst))
            }
            Keycode::Num8 => {
                self.specific_image.store(7, SeqCst);
                println!("specific image: {}", self.specific_image.load(SeqCst))
            }
            Keycode::Num9 => {
                self.specific_image.store(8, SeqCst);
                println!("specific image: {}", self.specific_image.load(SeqCst))
            }
            Keycode::Num0 => {
                self.specific_image.store(9, SeqCst);
                println!("specific image: all images")
            }
            _ => (),
        }
    }

    pub fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        image_descriptor_set: &Arc<DescriptorSet>,
        view_descriptor_set: &Arc<DescriptorSet>,
        light_fields: &[LightField],
        image: &Arc<Image>,
    ) -> VerboseResult<()> {
        self.update_buffer(command_buffer, light_fields)?;

        command_buffer.bind_pipeline(&self.pipeline)?;
        command_buffer.bind_descriptor_sets_minimal(&[
            &self.as_descriptor,
            view_descriptor_set,
            image_descriptor_set,
        ])?;
        command_buffer.trace_rays_sbt(&self.sbt, image.width(), image.height(), 1);

        command_buffer.set_full_image_layout(image, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR)?;

        Ok(())
    }

    fn update_buffer(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        light_fields: &[LightField],
    ) -> VerboseResult<()> {
        let light_field = Self::select_light_field(light_fields);
        let plane = self.select_plane(light_field);

        self.plane_buffer.fill(&[DebugPlane {
            top_left: plane.info.top_left,
            top_right: plane.info.top_right,
            bottom_left: plane.info.bottom_left,
            bottom_right: plane.info.bottom_right,

            normal: plane.info.normal,
        }])?;

        let image_info_buffer = Buffer::builder()
            .set_memory_properties(
                VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT,
            )
            .set_usage(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT)
            .set_data(&self.create_image_data(plane))
            .build(command_buffer.device().clone())?;

        let tlas = AccelerationStructure::top_level()
            .add_instance(&plane.blas, None, 0)
            .build(command_buffer.device().clone())?;

        tlas.generate(command_buffer)?;

        self.as_descriptor.update(&[
            DescriptorWrite::acceleration_structures(0, &[&tlas]),
            DescriptorWrite::storage_buffers(1, &[&self.plane_buffer]),
            DescriptorWrite::storage_buffers(2, &[&image_info_buffer]),
        ]);

        *self.tlas.lock()? = Some(tlas);
        *self.image_info_buffer.lock()? = image_info_buffer;

        Ok(())
    }

    fn select_light_field(light_fields: &[LightField]) -> &LightField {
        // assume single light field for now

        &light_fields[0]
    }

    fn select_plane<'a>(&self, light_field: &'a LightField) -> &'a Plane {
        let index = std::cmp::min(self.layer_index.load(SeqCst), light_field.planes.len() - 1);

        &light_field.planes[index]
    }

    fn create_image_data(&self, plane: &Plane) -> Vec<DebugImageInfo> {
        let index = self.specific_image.load(SeqCst);

        if index == 9 {
            plane
                .image_infos
                .iter()
                .map(|image_info| DebugImageInfo {
                    bound: image_info.ratios,
                    image_index: image_info.image_index,

                    padding: [0; 3],
                })
                .collect()
        } else {
            let image_info = &plane.image_infos[index];

            vec![DebugImageInfo {
                bound: image_info.ratios,
                image_index: image_info.image_index,

                padding: [0; 3],
            }]
        }
    }
}

#[derive(Debug, Clone)]
struct DebugPlane {
    top_left: Vector4<f32>,
    top_right: Vector4<f32>,
    bottom_left: Vector4<f32>,
    bottom_right: Vector4<f32>,

    normal: Vector4<f32>,
}

#[derive(Debug, Clone)]
struct DebugImageInfo {
    bound: PlaneImageRatios,
    image_index: u32,

    padding: [i32; 3],
}
