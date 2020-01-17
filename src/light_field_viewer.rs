use context::prelude::*;
use context::ContextObject;

use std::sync::{
    atomic::{AtomicU32, Ordering::SeqCst},
    Arc, Mutex,
};
use std::time::Duration;

use cgmath::{vec3, vec4, Deg, InnerSpace, Matrix4, SquareMatrix, Vector2, Vector3, Vector4};

use super::{
    interpolation::CPUInterpolation,
    light_field::{light_field_data::PlaneImageRatios, LightField},
    rasterizer::Rasterizer,
    view_emulator::ViewEmulator,
};

pub const DEFAULT_FORWARD: Vector3<f32> = vec3(0.0, 0.0, -1.0);
pub const UP: Vector3<f32> = vec3(0.0, 1.0, 0.0);

pub struct LightFieldViewer {
    context: Arc<Context>,

    view_buffers: TargetMode<Arc<Buffer<VRTransformations>>>,
    transform_descriptor: TargetMode<Arc<DescriptorSet>>,
    output_image_descriptor: TargetMode<Arc<DescriptorSet>>,
    as_descriptor: TargetMode<Arc<DescriptorSet>>,

    ray_tracing_pipeline: Arc<Pipeline>,
    sbt: ShaderBindingTable,

    // scene data
    _blas: Arc<AccelerationStructure>,
    _tlas: Arc<AccelerationStructure>,
    _images: Vec<Arc<Image>>,
    _vertex_buffer: Arc<Buffer<Vector3<f32>>>,
    plane_buffer: TargetMode<Arc<Buffer<PlaneInfo>>>,

    view_emulator: Mutex<ViewEmulator>,

    last_time_stemp: Mutex<Duration>,
    fps_count: AtomicU32,

    interpolation: CPUInterpolation,
    rasterizer: Rasterizer,
}

impl LightFieldViewer {
    pub fn new(
        context: &Arc<Context>,
        light_fields: Vec<LightField>,
        turn_speed: Deg<f32>,
        movement_speed: f32,
    ) -> VerboseResult<Arc<Self>> {
        let (blas, tlas, vertex_buffer, plane_buffer, images, interpolation) =
            Self::create_scene_data(context, light_fields)?;

        let view_buffers = Self::create_view_buffers(context)?;

        let transform_descriptor = Self::create_transform_descriptor(context, &view_buffers)?;

        let desc = match &transform_descriptor {
            TargetMode::Single(desc) => desc,
            TargetMode::Stereo(desc, _) => desc,
        };

        let device = context.device();

        let as_descriptor = Self::create_as_descriptor(device, &tlas, &plane_buffer, &images)?;

        let as_desc = match &as_descriptor {
            TargetMode::Single(desc) => desc,
            TargetMode::Stereo(desc, _) => desc,
        };

        let output_image_desc_layout = DescriptorSetLayout::builder()
            .add_layout_binding(
                0,
                VK_DESCRIPTOR_TYPE_STORAGE_IMAGE,
                VK_SHADER_STAGE_RAYGEN_BIT_NV,
                0,
            )
            .build(device.clone())?;

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
                None,
            )
            .add_shader(
                ShaderModule::from_slice(
                    device.clone(),
                    include_bytes!("../shader/miss.rmiss.spv"),
                    ShaderType::Miss,
                )?,
                None,
                None,
            )
            .add_hit_shaders(
                &[ShaderModule::from_slice(
                    device.clone(),
                    include_bytes!("../shader/closesthit.rchit.spv"),
                    ShaderType::ClosestHit,
                )?],
                None,
                vec![None],
            )
            .build(device, &[as_desc, desc, &output_image_desc_layout])?;

        context
            .render_core()
            .set_clear_color([0.2, 0.2, 0.2, 1.0])?;

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
            _vertex_buffer: vertex_buffer,
            plane_buffer,

            view_emulator: Mutex::new(ViewEmulator::new(context, turn_speed, movement_speed)),

            last_time_stemp: Mutex::new(context.time()),
            fps_count: AtomicU32::new(0),

            interpolation,
            rasterizer: Rasterizer::new(context)?,
        }))
    }
}

impl ContextObject for LightFieldViewer {
    fn name(&self) -> &str {
        "LightFieldViewer"
    }

    fn update(&self) -> VerboseResult<()> {
        self.view_emulator.lock()?.update()?;

        Ok(())
    }

    fn event(&self, event: PresentationEventType) -> VerboseResult<()> {
        // use `view_buffers` as reference
        if let TargetMode::Single(_) = self.view_buffers {
            match event {
                PresentationEventType::KeyDown(key) => match key {
                    Keycode::Escape => self.context.close()?,
                    _ => self.view_emulator.lock()?.on_key_down(key),
                },
                PresentationEventType::KeyUp(key) => self.view_emulator.lock()?.on_key_up(key),
                _ => (),
            }
        }

        Ok(())
    }
}

impl LightFieldViewer {
    fn update_view_buffer(
        view_buffer: &Arc<Buffer<VRTransformations>>,
        transform: VRTransformations,
    ) -> VerboseResult<()> {
        let mut mapped = view_buffer.map_complete()?;
        mapped[0] = transform;

        Ok(())
    }

    fn update_plane_buffer(
        plane_buffer: &Arc<Buffer<PlaneInfo>>,
        inverted_view: Matrix4<f32>,
        interpolation: &CPUInterpolation,
    ) -> VerboseResult<()> {
        interpolation.calculate_interpolation(inverted_view, plane_buffer.map_complete()?)?;

        Ok(())
    }
}

impl TScene for LightFieldViewer {
    fn update(&self) -> VerboseResult<()> {
        let current_time_stemp = self.context.time();
        self.fps_count.fetch_add(1, SeqCst);

        let last_time_stemp = *self.last_time_stemp.lock()?;

        if (current_time_stemp - last_time_stemp) >= Duration::from_secs_f32(1.0) {
            *self.last_time_stemp.lock()? = last_time_stemp + Duration::from_secs_f32(1.0);

            println!("fps: {}", self.fps_count.load(SeqCst));
            self.fps_count.store(0, SeqCst);
        }

        match (&self.view_buffers, &self.plane_buffer) {
            (TargetMode::Single(view_buffer), TargetMode::Single(plane_buffer)) => {
                let inverted_transform =
                    self.view_emulator.lock()?.simulation_transform().invert()?;

                Self::update_view_buffer(view_buffer, inverted_transform)?;
                Self::update_plane_buffer(
                    plane_buffer,
                    inverted_transform.view,
                    &self.interpolation,
                )?;
            }
            (
                TargetMode::Stereo(left_view_buffer, right_view_buffer),
                TargetMode::Stereo(left_plane_buffer, right_plane_buffer),
            ) => {
                let (left_transform, right_transform) = self
                    .context
                    .render_core()
                    .transformations()?
                    .ok_or("expected vr transformations")?;

                let left_inverted_transform = left_transform.invert()?;
                Self::update_view_buffer(left_view_buffer, left_inverted_transform)?;
                Self::update_plane_buffer(
                    left_plane_buffer,
                    left_inverted_transform.view,
                    &self.interpolation,
                )?;

                let right_inverted_transform = right_transform.invert()?;
                Self::update_view_buffer(right_view_buffer, right_inverted_transform)?;
                Self::update_plane_buffer(
                    right_plane_buffer,
                    right_inverted_transform.view,
                    &self.interpolation,
                )?;
            }
            _ => create_error!("wrong setup"),
        }

        Ok(())
    }

    fn process(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        indices: &TargetMode<usize>,
    ) -> VerboseResult<()> {
        match (
            indices,
            &self.transform_descriptor,
            &self.as_descriptor,
            &self.output_image_descriptor,
            &self.context.render_core().images()?,
        ) {
            (
                TargetMode::Single(index),
                TargetMode::Single(example_descriptor),
                TargetMode::Single(as_descriptor),
                TargetMode::Single(image_descriptor),
                TargetMode::Single(target_images),
            ) => {
                self.render(
                    *index,
                    command_buffer,
                    example_descriptor,
                    as_descriptor,
                    target_images,
                    image_descriptor,
                )?;
            }
            (
                TargetMode::Stereo(left_index, right_index),
                TargetMode::Stereo(left_descriptor, right_descriptor),
                TargetMode::Stereo(left_as_descriptor, right_as_descriptor),
                TargetMode::Stereo(left_image_descriptor, right_image_descriptor),
                TargetMode::Stereo(left_image, right_image),
            ) => {
                self.render(
                    *left_index,
                    command_buffer,
                    left_descriptor,
                    left_as_descriptor,
                    left_image,
                    left_image_descriptor,
                )?;

                self.render(
                    *right_index,
                    command_buffer,
                    right_descriptor,
                    right_as_descriptor,
                    right_image,
                    right_image_descriptor,
                )?;
            }
            _ => create_error!("invalid target mode setup"),
        }

        Ok(())
    }

    fn resize(&self) -> VerboseResult<()> {
        self.view_emulator.lock()?.on_resize();

        Ok(())
    }
}

impl LightFieldViewer {
    fn render(
        &self,
        index: usize,
        command_buffer: &Arc<CommandBuffer>,
        view_descriptor_set: &Arc<DescriptorSet>,
        as_descriptor_set: &Arc<DescriptorSet>,
        images: &Vec<Arc<Image>>,
        image_descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        let image = &images[index];
        image_descriptor.update(&[DescriptorWrite::storage_images(0, &[image])
            .change_image_layout(VK_IMAGE_LAYOUT_GENERAL)]);

        command_buffer.set_full_image_layout(image, VK_IMAGE_LAYOUT_GENERAL)?;

        command_buffer.bind_pipeline(&self.ray_tracing_pipeline)?;
        command_buffer.bind_descriptor_sets_minimal(&[
            as_descriptor_set,
            view_descriptor_set,
            image_descriptor,
        ])?;
        command_buffer.trace_rays_sbt(&self.sbt, image.width(), image.height(), 1);

        command_buffer.set_full_image_layout(image, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR)?;

        Ok(())
    }

    fn create_view_buffers(
        context: &Arc<Context>,
    ) -> VerboseResult<TargetMode<Arc<Buffer<VRTransformations>>>> {
        let render_core = context.render_core();

        match render_core.images()? {
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

        match render_core.images()? {
            TargetMode::Single(_) => Ok(TargetMode::Single(create_desc()?)),
            TargetMode::Stereo(_, _) => Ok(TargetMode::Stereo(create_desc()?, create_desc()?)),
        }
    }

    fn create_as_descriptor(
        device: &Arc<Device>,
        tlas: &Arc<AccelerationStructure>,
        plane_buffer: &TargetMode<Arc<Buffer<PlaneInfo>>>,
        images: &Vec<Arc<Image>>,
    ) -> VerboseResult<TargetMode<Arc<DescriptorSet>>> {
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
                VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER,
                VK_SHADER_STAGE_CLOSEST_HIT_BIT_NV,
                VK_DESCRIPTOR_BINDING_VARIABLE_DESCRIPTOR_COUNT_BIT_EXT,
            )
            .change_descriptor_count(images.len() as u32)
            .build(device.clone())?;

        let image_refs: Vec<&Arc<Image>> = images.iter().map(|image| image).collect();

        match plane_buffer {
            TargetMode::Single(plane_buffer) => {
                let descriptor_pool = DescriptorPool::builder()
                    .set_layout(descriptor_set_layout)
                    .build(device.clone())?;

                let descriptor_set = DescriptorPool::prepare_set(&descriptor_pool).allocate()?;

                descriptor_set.update(&[
                    DescriptorWrite::acceleration_structures(0, &[tlas]),
                    DescriptorWrite::storage_buffers(1, &[plane_buffer]),
                    DescriptorWrite::combined_samplers(2, &image_refs),
                ]);

                Ok(TargetMode::Single(descriptor_set))
            }
            TargetMode::Stereo(left_plane_buffer, right_plane_buffer) => {
                let left_desc_pool = DescriptorPool::builder()
                    .set_layout(descriptor_set_layout.clone())
                    .build(device.clone())?;

                let left_desc_set = DescriptorPool::prepare_set(&left_desc_pool).allocate()?;

                left_desc_set.update(&[
                    DescriptorWrite::acceleration_structures(0, &[tlas]),
                    DescriptorWrite::storage_buffers(1, &[left_plane_buffer]),
                    DescriptorWrite::combined_samplers(2, &image_refs),
                ]);

                let right_desc_pool = DescriptorPool::builder()
                    .set_layout(descriptor_set_layout)
                    .build(device.clone())?;

                let right_desc_set = DescriptorPool::prepare_set(&right_desc_pool).allocate()?;

                right_desc_set.update(&[
                    DescriptorWrite::acceleration_structures(0, &[tlas]),
                    DescriptorWrite::storage_buffers(1, &[right_plane_buffer]),
                    DescriptorWrite::combined_samplers(2, &image_refs),
                ]);

                Ok(TargetMode::Stereo(left_desc_set, right_desc_set))
            }
        }
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
        Arc<Buffer<Vector3<f32>>>,
        TargetMode<Arc<Buffer<PlaneInfo>>>,
        Vec<Arc<Image>>,
        CPUInterpolation,
    )> {
        let mut vertex_data = Vec::new();
        let mut plane_infos = Vec::new();
        let mut interpolation_infos = Vec::new();
        let mut images = Vec::new();

        while let Some(light_field) = light_fields.pop() {
            let frustum = light_field.frustum();
            let mut planes = light_field.into_data();

            while let Some(mut plane) = planes.pop() {
                let mut image_infos = Vec::with_capacity(plane.content.len());

                // add plane contents to buffers
                while let Some((image, ratios, center)) = plane.content.pop() {
                    // get image index and add image
                    let image_index = images.len() as u32;
                    images.push(image);

                    image_infos.push(PlaneImageInfo {
                        ratios,
                        center,
                        image_index,
                    });
                }

                let plane_normal = (plane.left_top - plane.left_bottom)
                    .cross(plane.left_bottom - plane.right_bottom)
                    .normalize();

                let plane_info = PlaneInfo {
                    top_left: plane.left_top.extend(0.0),
                    top_right: plane.right_top.extend(0.0),
                    bottom_left: plane.left_bottom.extend(0.0),
                    bottom_right: plane.right_bottom.extend(0.0),

                    normal: plane_normal.extend(0.0),

                    indices: vec4(-1, -1, -1, -1),
                    weights: vec4(0.0, 0.0, 0.0, 0.0),
                };

                plane_infos.push(plane_info.clone());
                interpolation_infos.push((plane_info, frustum.clone(), image_infos));

                // create vertex data
                // v0
                vertex_data.push(plane.left_bottom);

                // v1
                vertex_data.push(plane.left_top);

                // v2
                vertex_data.push(plane.right_bottom);

                // v3
                vertex_data.push(plane.right_bottom);

                // v4
                vertex_data.push(plane.left_top);

                // v5
                vertex_data.push(plane.right_top);
            }
        }

        let command_buffer = context.render_core().allocate_primary_buffer()?;

        // --- create vertex buffer ---
        let vertex_cpu_buffer = Buffer::builder()
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_usage(VK_BUFFER_USAGE_RAY_TRACING_BIT_NV | VK_BUFFER_USAGE_TRANSFER_SRC_BIT)
            .set_data(&vertex_data)
            .build(context.device().clone())?;

        let vertex_gpu_buffer =
            Buffer::into_device_local(vertex_cpu_buffer, &command_buffer, context.queue())?;

        // --- create plane info buffer ---
        let plane_buffer = match context.render_core().images()? {
            TargetMode::Single(_) => TargetMode::Single(
                Buffer::builder()
                    .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
                    .set_usage(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT)
                    .set_data(&plane_infos)
                    .build(context.device().clone())?,
            ),
            TargetMode::Stereo(_, _) => TargetMode::Stereo(
                Buffer::builder()
                    .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
                    .set_usage(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT)
                    .set_data(&plane_infos)
                    .build(context.device().clone())?,
                Buffer::builder()
                    .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
                    .set_usage(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT)
                    .set_data(&plane_infos)
                    .build(context.device().clone())?,
            ),
        };

        Buffer::builder()
            .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
            .set_usage(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT)
            .set_data(&plane_infos)
            .build(context.device().clone())?;

        // --- create acceleration structures ---
        let blas = AccelerationStructure::bottom_level()
            .set_flags(VK_BUILD_ACCELERATION_STRUCTURE_PREFER_FAST_TRACE_BIT_NV)
            .add_vertices(&vertex_gpu_buffer, None)
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

        Ok((
            blas,
            tlas,
            vertex_gpu_buffer,
            plane_buffer,
            images,
            CPUInterpolation::new(interpolation_infos),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct PlaneInfo {
    pub top_left: Vector4<f32>,
    pub top_right: Vector4<f32>,
    pub bottom_left: Vector4<f32>,
    pub bottom_right: Vector4<f32>,

    pub normal: Vector4<f32>,

    pub indices: Vector4<i32>,
    pub weights: Vector4<f32>,
}

#[derive(Debug, Clone)]
pub struct PlaneImageInfo {
    pub ratios: PlaneImageRatios,
    pub center: Vector2<f32>,
    pub image_index: u32,
}
