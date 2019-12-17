use context::prelude::*;
use context::ContextObject;

use std::cell::Cell;
use std::sync::Arc;

use cgmath::{vec3, Deg, InnerSpace, Matrix4, SquareMatrix, Vector2, Vector3, Vector4};

use super::{
    light_field::{light_field_data::PlaneImageRatios, LightField},
    view_emulator::ViewEmulator,
};

pub const DEFAULT_FORWARD: Vector3<f32> = vec3(0.0, 0.0, -1.0);
pub const UP: Vector3<f32> = vec3(0.0, 1.0, 0.0);

pub struct LightFieldViewer {
    context: Arc<Context>,

    view_buffers: TargetMode<Arc<Buffer<VRTransformations>>>,
    transform_descriptor: TargetMode<Arc<DescriptorSet>>,
    output_image_descriptor: TargetMode<Arc<DescriptorSet>>,
    as_descriptor: Arc<DescriptorSet>,

    ray_tracing_pipeline: Arc<Pipeline>,
    sbt: ShaderBindingTable,

    // scene data
    _blas: Arc<AccelerationStructure>,
    _tlas: Arc<AccelerationStructure>,
    _images: Vec<Arc<Image>>,
    _primary_buffer: Arc<Buffer<PlaneVertex>>,
    _secondary_buffer: Arc<Buffer<PlaneImageInfo>>,

    view_emulator: ViewEmulator,

    last_time_stemp: Cell<f64>,
    fps_count: Cell<u32>,
}

impl LightFieldViewer {
    pub fn new(context: &Arc<Context>, light_fields: Vec<LightField>) -> VerboseResult<Arc<Self>> {
        let (blas, tlas, primary_buffer, secondary_buffer, images) =
            Self::create_scene_data(context, light_fields)?;

        let view_buffers = Self::create_view_buffers(context)?;

        let transform_descriptor = Self::create_transform_descriptor(context, &view_buffers)?;

        let desc = match &transform_descriptor {
            TargetMode::Single(desc) => desc,
            TargetMode::Stereo(desc, _) => desc,
        };

        let device = context.device();

        let as_descriptor = Self::create_as_descriptor(
            context.device(),
            &tlas,
            &primary_buffer,
            &secondary_buffer,
        )?;

        let output_image_desc_layout = DescriptorSetLayout::builder()
            .add_layout_binding(
                0,
                VK_DESCRIPTOR_TYPE_STORAGE_IMAGE,
                VK_SHADER_STAGE_RAYGEN_BIT_NV,
                0,
            )
            .build(context.device().clone())?;

        let output_image_descriptor =
            Self::create_output_image_descriptor(context, &output_image_desc_layout)?;

        let (pipeline, sbt) = Pipeline::new_ray_tracing()
            .add_shader(
                ShaderModule::from_slice(
                    device.clone(),
                    include_bytes!("../shader/raygen.rgen.spv"),
                    ShaderType::RayGeneration,
                )?,
                None,
            )
            .add_shader(
                ShaderModule::from_slice(
                    device.clone(),
                    include_bytes!("../shader/miss.rmiss.spv"),
                    ShaderType::Miss,
                )?,
                None,
            )
            .add_hit_shaders(
                &[ShaderModule::from_slice(
                    device.clone(),
                    include_bytes!("../shader/closesthit.rchit.spv"),
                    ShaderType::ClosestHit,
                )?],
                None,
            )
            .build(device, &[&as_descriptor, desc, &output_image_desc_layout])?;

        Ok(Arc::new(LightFieldViewer {
            context: context.clone(),

            view_buffers,
            transform_descriptor,
            output_image_descriptor,
            as_descriptor,

            ray_tracing_pipeline: pipeline,
            sbt,

            _blas: blas,
            _tlas: tlas,
            _images: images,
            _primary_buffer: primary_buffer,
            _secondary_buffer: secondary_buffer,

            view_emulator: ViewEmulator::new(context, Deg(45.0), 2.5),

            last_time_stemp: Cell::new(context.time()),
            fps_count: Cell::new(0),
        }))
    }
}

impl ContextObject for LightFieldViewer {
    fn name(&self) -> &str {
        "LightFieldViewer"
    }

    fn update(&self) -> VerboseResult<()> {
        self.view_emulator.update()?;

        Ok(())
    }

    fn event(&self, event: PresentationEventType) -> VerboseResult<()> {
        // use `view_buffers` as reference
        if let TargetMode::Single(_) = self.view_buffers {
            match event {
                PresentationEventType::KeyDown(key) => match key {
                    Keycode::Escape => self.context.close(),
                    _ => self.view_emulator.on_key_down(key),
                },
                PresentationEventType::KeyUp(key) => self.view_emulator.on_key_up(key),
                _ => (),
            }
        }

        Ok(())
    }
}

impl TScene for LightFieldViewer {
    fn update(&self) -> VerboseResult<()> {
        let current_time_stemp = self.context.time();
        self.fps_count.set(self.fps_count.get() + 1);

        if (current_time_stemp - self.last_time_stemp.get()) >= 1.0 {
            self.last_time_stemp.set(self.last_time_stemp.get() + 1.0);

            println!("fps: {}", self.fps_count.get());
            self.fps_count.set(0);
        }

        Ok(())
    }

    fn process(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        indices: &TargetMode<usize>,
        transforms: &Option<TargetMode<VRTransformations>>,
    ) -> VerboseResult<()> {
        match (
            indices,
            &self.view_buffers,
            &self.transform_descriptor,
            &self.output_image_descriptor,
            &self.context.render_core().images(),
        ) {
            (
                TargetMode::Single(index),
                TargetMode::Single(view_buffer),
                TargetMode::Single(example_descriptor),
                TargetMode::Single(image_descriptor),
                TargetMode::Single(target_images),
            ) => {
                self.render(
                    *index,
                    command_buffer,
                    view_buffer,
                    &self.view_emulator.simulation_transform(),
                    example_descriptor,
                    target_images,
                    image_descriptor,
                )?;
            }
            (
                TargetMode::Stereo(left_index, right_index),
                TargetMode::Stereo(left_view_buffer, right_view_buffer),
                TargetMode::Stereo(left_descriptor, right_descriptor),
                TargetMode::Stereo(left_image_descriptor, right_image_descriptor),
                TargetMode::Stereo(left_image, right_image),
            ) => {
                let (left_transform, right_transform) = transforms
                    .as_ref()
                    .ok_or("no transforms present")?
                    .stereo()?;

                self.render(
                    *left_index,
                    command_buffer,
                    left_view_buffer,
                    left_transform,
                    left_descriptor,
                    left_image,
                    left_image_descriptor,
                )?;

                self.render(
                    *right_index,
                    command_buffer,
                    right_view_buffer,
                    right_transform,
                    right_descriptor,
                    right_image,
                    right_image_descriptor,
                )?;
            }
            _ => create_error!("invalid target mode setup"),
        }

        Ok(())
    }

    fn resize(&self) -> VerboseResult<()> {
        self.view_emulator.on_resize();

        Ok(())
    }
}

impl LightFieldViewer {
    fn render(
        &self,
        index: usize,
        command_buffer: &Arc<CommandBuffer>,
        view_buffer: &Arc<Buffer<VRTransformations>>,
        transform: &VRTransformations,
        view_descriptor_set: &Arc<DescriptorSet>,
        images: &Vec<Arc<Image>>,
        image_descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        // update
        {
            let mut mapped = view_buffer.map_complete()?;
            mapped[0] = VRTransformations {
                proj: transform
                    .proj
                    .invert()
                    .expect("could not invert projection matrix"),
                view: transform
                    .view
                    .invert()
                    .expect("could not invert view matrix"),
            };
        }

        let image = &images[index];
        image_descriptor.update(&[DescriptorWrite::storage_images(0, &[image])
            .change_image_layout(VK_IMAGE_LAYOUT_GENERAL)]);

        command_buffer.set_full_image_layout(image, VK_IMAGE_LAYOUT_GENERAL);

        command_buffer.bind_pipeline(&self.ray_tracing_pipeline)?;
        command_buffer.bind_descriptor_sets_minimal(&[
            &self.as_descriptor,
            view_descriptor_set,
            image_descriptor,
        ])?;
        command_buffer.trace_rays_sbt(&self.sbt, image.width(), image.height(), 1);

        command_buffer.set_full_image_layout(image, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR);

        Ok(())
    }

    fn create_view_buffers(
        context: &Arc<Context>,
    ) -> VerboseResult<TargetMode<Arc<Buffer<VRTransformations>>>> {
        let render_core = context.render_core();

        match render_core.images() {
            TargetMode::Single(_) => Ok(TargetMode::Single(
                Buffer::builder()
                    .set_usage(VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT)
                    .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
                    .set_size(1)
                    .build(context.device().clone())?,
            )),
            TargetMode::Stereo(_, _) => Ok(TargetMode::Stereo(
                Buffer::builder()
                    .set_usage(VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT)
                    .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
                    .set_size(1)
                    .build(context.device().clone())?,
                Buffer::builder()
                    .set_usage(VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT)
                    .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
                    .set_size(1)
                    .build(context.device().clone())?,
            )),
        }
    }

    fn create_output_image_descriptor(
        context: &Arc<Context>,
        layout: &Arc<DescriptorSetLayout>,
    ) -> VerboseResult<TargetMode<Arc<DescriptorSet>>> {
        let render_core = context.render_core();

        let create_desc = || {
            let pool = DescriptorPool::builder()
                .set_layout(layout.clone())
                .build(context.device().clone())?;

            DescriptorPool::prepare_set(&pool).allocate()
        };

        match render_core.images() {
            TargetMode::Single(_) => Ok(TargetMode::Single(create_desc()?)),
            TargetMode::Stereo(_, _) => Ok(TargetMode::Stereo(create_desc()?, create_desc()?)),
        }
    }

    fn create_as_descriptor(
        device: &Arc<Device>,
        tlas: &Arc<AccelerationStructure>,
        primary_buffer: &Arc<Buffer<PlaneVertex>>,
        secondary_buffer: &Arc<Buffer<PlaneImageInfo>>,
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
            .build(device.clone())?;

        let descriptor_pool = DescriptorPool::builder()
            .set_layout(descriptor_set_layout)
            .build(device.clone())?;

        let descriptor_set = DescriptorPool::prepare_set(&descriptor_pool).allocate()?;

        descriptor_set.update(&[
            DescriptorWrite::acceleration_structures(0, &[tlas]),
            DescriptorWrite::storage_buffers(1, &[primary_buffer]),
            DescriptorWrite::storage_buffers(2, &[secondary_buffer]),
        ]);

        Ok(descriptor_set)
    }

    fn create_transform_descriptor(
        context: &Arc<Context>,
        view_buffers: &TargetMode<Arc<Buffer<VRTransformations>>>,
    ) -> VerboseResult<TargetMode<Arc<DescriptorSet>>> {
        let descriptor_layout = DescriptorSetLayout::builder()
            .add_layout_binding(
                0,
                VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER,
                VK_SHADER_STAGE_RAYGEN_BIT_NV,
                0,
            )
            .build(context.device().clone())?;

        match view_buffers {
            TargetMode::Single(view_buffer) => {
                let descriptor_set =
                    Self::create_descriptor_set(context.device(), &descriptor_layout)?;

                descriptor_set.update(&[DescriptorWrite::uniform_buffers(0, &[view_buffer])]);

                Ok(TargetMode::Single(descriptor_set))
            }
            TargetMode::Stereo(left_view_buffer, right_view_buffer) => {
                let left_descriptor =
                    Self::create_descriptor_set(context.device(), &descriptor_layout)?;
                let right_descriptor =
                    Self::create_descriptor_set(context.device(), &descriptor_layout)?;

                left_descriptor.update(&[DescriptorWrite::uniform_buffers(0, &[left_view_buffer])]);
                right_descriptor
                    .update(&[DescriptorWrite::uniform_buffers(0, &[right_view_buffer])]);

                Ok(TargetMode::Stereo(left_descriptor, right_descriptor))
            }
        }
    }

    fn create_descriptor_set(
        device: &Arc<Device>,
        layout: &Arc<DescriptorSetLayout>,
    ) -> VerboseResult<Arc<DescriptorSet>> {
        let descriptor_pool = DescriptorPool::builder()
            .set_layout(layout.clone())
            .build(device.clone())?;

        Ok(DescriptorPool::prepare_set(&descriptor_pool).allocate()?)
    }

    fn create_scene_data(
        context: &Arc<Context>,
        mut light_fields: Vec<LightField>,
    ) -> VerboseResult<(
        Arc<AccelerationStructure>,
        Arc<AccelerationStructure>,
        Arc<Buffer<PlaneVertex>>,
        Arc<Buffer<PlaneImageInfo>>,
        Vec<Arc<Image>>,
    )> {
        let mut primary_data = Vec::new();
        let mut secondary_data = Vec::new();
        let mut images = Vec::new();

        while let Some(light_field) = light_fields.pop() {
            let mut planes = light_field.into_data();

            while let Some(mut plane) = planes.pop() {
                // get first index
                let first_index = secondary_data.len();

                // add plane contents to buffers
                while let Some((image, ratios, center)) = plane.content.pop() {
                    // get image index and add image
                    let image_index = images.len() as u32;
                    images.push(image);

                    secondary_data.push(PlaneImageInfo {
                        ratios,
                        center,
                        image_index,

                        padding: [0],
                    });
                }

                // get last index
                let last_index = secondary_data.len();

                let plane_normal = (plane.left_top - plane.left_bottom)
                    .cross(plane.left_bottom - plane.right_bottom)
                    .normalize();

                // create vertex data
                // v0
                primary_data.push(PlaneVertex {
                    position_first: plane.left_bottom.extend(first_index as f32),
                    normal_last: plane_normal.extend(last_index as f32),
                });

                // v1
                primary_data.push(PlaneVertex {
                    position_first: plane.left_top.extend(first_index as f32),
                    normal_last: plane_normal.extend(last_index as f32),
                });

                // v2
                primary_data.push(PlaneVertex {
                    position_first: plane.right_bottom.extend(first_index as f32),
                    normal_last: plane_normal.extend(last_index as f32),
                });

                // v3
                primary_data.push(PlaneVertex {
                    position_first: plane.right_bottom.extend(first_index as f32),
                    normal_last: plane_normal.extend(last_index as f32),
                });

                // v4
                primary_data.push(PlaneVertex {
                    position_first: plane.left_top.extend(first_index as f32),
                    normal_last: plane_normal.extend(last_index as f32),
                });

                // v5
                primary_data.push(PlaneVertex {
                    position_first: plane.right_top.extend(first_index as f32),
                    normal_last: plane_normal.extend(last_index as f32),
                });
            }
        }

        let command_buffer = context.render_core().allocate_primary_buffer()?;

        let primary_cpu_buffer = Buffer::builder()
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_usage(
                VK_BUFFER_USAGE_RAY_TRACING_BIT_NV
                    | VK_BUFFER_USAGE_STORAGE_BUFFER_BIT
                    | VK_BUFFER_USAGE_TRANSFER_SRC_BIT,
            )
            .set_data(&primary_data)
            .build(context.device().clone())?;

        let primary_gpu_buffer =
            Buffer::into_device_local(primary_cpu_buffer, &command_buffer, context.queue())?;

        let secondary_cpu_buffer = Buffer::builder()
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_usage(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT | VK_BUFFER_USAGE_TRANSFER_SRC_BIT)
            .set_data(&secondary_data)
            .build(context.device().clone())?;

        let secondary_gpu_buffer =
            Buffer::into_device_local(secondary_cpu_buffer, &command_buffer, context.queue())?;

        let blas = AccelerationStructure::bottom_level()
            .set_flags(VK_BUILD_ACCELERATION_STRUCTURE_PREFER_FAST_TRACE_BIT_NV)
            .add_vertices(&primary_gpu_buffer, None)
            .build(context.device().clone())?;

        let tlas = AccelerationStructure::top_level()
            .set_flags(VK_BUILD_ACCELERATION_STRUCTURE_PREFER_FAST_TRACE_BIT_NV)
            .add_instance(
                &blas,
                Matrix4::identity(),
                VK_GEOMETRY_INSTANCE_TRIANGLE_CULL_DISABLE_BIT_NV,
            )
            .build(context.device().clone())?;

        command_buffer.begin(VkCommandBufferBeginInfo::new(
            VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
        ))?;

        blas.generate(&command_buffer)?;
        tlas.generate(&command_buffer)?;

        command_buffer.end()?;

        let submit = SubmitInfo::default().add_command_buffer(&command_buffer);
        let fence = Fence::builder().build(context.device().clone())?;

        let queue_lock = context.queue().lock()?;
        queue_lock.submit(Some(&fence), &[submit])?;

        context
            .device()
            .wait_for_fences(&[&fence], true, 1_000_000_000)?;

        Ok((blas, tlas, primary_gpu_buffer, secondary_gpu_buffer, images))
    }
}

// Indices are [First Index; Last Index[
#[derive(Debug, Clone)]
pub struct PlaneVertex {
    // (Position (vec3), First Index (f32))
    position_first: Vector4<f32>,

    // (Normal (vec3), Last Index (f32))
    normal_last: Vector4<f32>,
}

#[derive(Debug, Clone)]
pub struct PlaneImageInfo {
    // 4 * f32 = 16 Byte
    ratios: PlaneImageRatios,

    // 2 * f32 = 8 Byte
    center: Vector2<f32>,

    // u32 = 4 Byte
    image_index: u32,

    // 4 padding Byte are needed
    padding: [u32; 1],
}
