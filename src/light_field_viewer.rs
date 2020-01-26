use context::prelude::*;
use context::ContextObject;

use std::sync::{
    atomic::{AtomicU32, Ordering::SeqCst},
    Arc, Mutex,
};
use std::time::Duration;

use cgmath::{vec3, vec4, Deg, InnerSpace, Matrix4, SquareMatrix, Vector2, Vector3, Vector4};

use super::{
    feet_renderer::FeetRenderer,
    interpolation::CPUInterpolation,
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
    as_descriptor: TargetMode<Arc<DescriptorSet>>,

    ray_tracing_pipeline: Arc<Pipeline>,
    sbt: ShaderBindingTable,

    // scene data
    acceleration_structures: Mutex<
        Option<TargetMode<Option<(Vec<Arc<AccelerationStructure>>, Arc<AccelerationStructure>)>>>,
    >,
    _images: Vec<Arc<Image>>,
    plane_buffer: TargetMode<Arc<Buffer<PlaneInfo>>>,

    view_emulator: Mutex<ViewEmulator>,

    last_time_stamp: Mutex<Duration>,
    fps_count: AtomicU32,

    interpolation: CPUInterpolation,
    feet: FeetRenderer,
}

impl LightFieldViewer {
    pub fn new(
        context: &Arc<Context>,
        light_fields: Vec<LightField>,
        turn_speed: Deg<f32>,
        movement_speed: f32,
    ) -> VerboseResult<Arc<Self>> {
        let feet_renderer = FeetRenderer::new(context, &light_fields)?;

        let (plane_buffer, images, interpolation) = Self::create_scene_data(context, light_fields)?;

        let view_buffers = Self::create_view_buffers(context)?;

        let transform_descriptor = Self::create_transform_descriptor(context, &view_buffers)?;

        let desc = match &transform_descriptor {
            TargetMode::Single(desc) => desc,
            TargetMode::Stereo(desc, _) => desc,
        };

        let device = context.device();

        let as_descriptor = Self::create_as_descriptor(device, &plane_buffer, &images)?;

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

        let pipeline_layout = PipelineLayout::builder()
            .add_descriptor_set_layout(as_desc)
            .add_descriptor_set_layout(desc)
            .add_descriptor_set_layout(&output_image_desc_layout)
            .build(device.clone())?;

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
                vec![ShaderModule::from_slice(
                    device.clone(),
                    include_bytes!("../shader/closesthit.rchit.spv"),
                    ShaderType::ClosestHit,
                )?],
                None,
                vec![None],
            )
            .build(device, &pipeline_layout)?;

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

            acceleration_structures: Mutex::new(None),
            _images: images,
            plane_buffer,

            view_emulator: Mutex::new(ViewEmulator::new(context, turn_speed, movement_speed)),

            last_time_stamp: Mutex::new(context.time()),
            fps_count: AtomicU32::new(0),

            interpolation,
            feet: feet_renderer,
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

impl TScene for LightFieldViewer {
    fn update(&self) -> VerboseResult<()> {
        let current_time_stamp = self.context.time();
        self.fps_count.fetch_add(1, SeqCst);

        let last_time_stamp = *self.last_time_stamp.lock()?;

        if (current_time_stamp - last_time_stamp) >= Duration::from_secs_f32(1.0) {
            *self.last_time_stamp.lock()? = last_time_stamp + Duration::from_secs_f32(1.0);

            // println!("fps: {}", self.fps_count.load(SeqCst));
            self.fps_count.store(0, SeqCst);
        }

        match &self.view_buffers {
            TargetMode::Single(view_buffer) => {
                let inverted_transform =
                    self.view_emulator.lock()?.simulation_transform().invert()?;

                Self::update_view_buffer(view_buffer, inverted_transform)?;
            }

            TargetMode::Stereo(left_view_buffer, right_view_buffer) => {
                let (left_transform, right_transform) = self
                    .context
                    .render_core()
                    .transformations()?
                    .ok_or("expected vr transformations")?;

                let left_inverted_transform = left_transform.invert()?;
                Self::update_view_buffer(left_view_buffer, left_inverted_transform)?;

                let right_inverted_transform = right_transform.invert()?;
                Self::update_view_buffer(right_view_buffer, right_inverted_transform)?;
            }
        }

        Ok(())
    }

    fn process(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        indices: &TargetMode<usize>,
    ) -> VerboseResult<()> {
        let rasterizer = self.feet.rasterizer()?;

        match (
            indices,
            &self.transform_descriptor,
            &self.as_descriptor,
            &self.output_image_descriptor,
            &self.context.render_core().images()?,
            rasterizer.triangle_pipelines(),
            rasterizer.line_pipelines(),
            rasterizer.render_targets(),
            &self.plane_buffer,
        ) {
            (
                TargetMode::Single(index),
                TargetMode::Single(example_descriptor),
                TargetMode::Single(as_descriptor),
                TargetMode::Single(image_descriptor),
                TargetMode::Single(target_images),
                TargetMode::Single(feet_pipeline),
                TargetMode::Single(line_pipeline),
                TargetMode::Single(feet_render_target),
                TargetMode::Single(plane_buffer),
            ) => {
                let transform = self.view_emulator.lock()?.simulation_transform();

                self.feet.render(
                    command_buffer,
                    feet_pipeline,
                    line_pipeline,
                    feet_render_target,
                    *index,
                    transform,
                )?;

                if let Some((blasses, tlas)) = Self::update_plane_buffer(
                    command_buffer,
                    &self.context,
                    plane_buffer,
                    transform
                        .view
                        .invert()
                        .ok_or("failed inverting simulation view")?,
                    &self.interpolation,
                )? {
                    as_descriptor.update(&[DescriptorWrite::acceleration_structures(0, &[&tlas])]);

                    *self.acceleration_structures.lock()? =
                        Some(TargetMode::Single(Some((blasses, tlas))));

                    self.render(
                        *index,
                        command_buffer,
                        example_descriptor,
                        as_descriptor,
                        target_images,
                        image_descriptor,
                    )?;
                }
            }
            (
                TargetMode::Stereo(left_index, right_index),
                TargetMode::Stereo(left_descriptor, right_descriptor),
                TargetMode::Stereo(left_as_descriptor, right_as_descriptor),
                TargetMode::Stereo(left_image_descriptor, right_image_descriptor),
                TargetMode::Stereo(left_image, right_image),
                TargetMode::Stereo(left_feet_pipeline, right_feet_pipeline),
                TargetMode::Stereo(left_line_pipeline, right_line_pipeline),
                TargetMode::Stereo(left_feet_render_target, right_feet_render_target),
                TargetMode::Stereo(left_plane_buffer, right_plane_buffer),
            ) => {
                let (left_transform, right_transform) = self
                    .context
                    .render_core()
                    .transformations()?
                    .ok_or("expected vr transformations")?;

                // render feet
                self.feet.render(
                    command_buffer,
                    left_feet_pipeline,
                    left_line_pipeline,
                    left_feet_render_target,
                    *left_index,
                    left_transform,
                )?;

                self.feet.render(
                    command_buffer,
                    right_feet_pipeline,
                    right_line_pipeline,
                    right_feet_render_target,
                    *right_index,
                    right_transform,
                )?;

                if let Some((left_blasses, left_tlas)) = Self::update_plane_buffer(
                    command_buffer,
                    &self.context,
                    left_plane_buffer,
                    left_transform
                        .view
                        .invert()
                        .ok_or("failed inverting left view")?,
                    &self.interpolation,
                )? {
                    left_as_descriptor
                        .update(&[DescriptorWrite::acceleration_structures(0, &[&left_tlas])]);

                    self.update_left_as(left_blasses, left_tlas)?;

                    self.render(
                        *left_index,
                        command_buffer,
                        left_descriptor,
                        left_as_descriptor,
                        left_image,
                        left_image_descriptor,
                    )?;
                }

                if let Some((right_blasses, right_tlas)) = Self::update_plane_buffer(
                    command_buffer,
                    &self.context,
                    right_plane_buffer,
                    right_transform
                        .view
                        .invert()
                        .ok_or("failed inverting right view")?,
                    &self.interpolation,
                )? {
                    right_as_descriptor
                        .update(&[DescriptorWrite::acceleration_structures(0, &[&right_tlas])]);

                    self.update_right_as(right_blasses, right_tlas)?;

                    self.render(
                        *right_index,
                        command_buffer,
                        right_descriptor,
                        right_as_descriptor,
                        right_image,
                        right_image_descriptor,
                    )?;
                }
            }
            _ => create_error!("invalid target mode setup"),
        }

        Ok(())
    }

    fn resize(&self) -> VerboseResult<()> {
        self.view_emulator.lock()?.on_resize();

        self.feet.on_resize(&self.context)?;

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
                    DescriptorWrite::storage_buffers(1, &[left_plane_buffer]),
                    DescriptorWrite::combined_samplers(2, &image_refs),
                ]);

                let right_desc_pool = DescriptorPool::builder()
                    .set_layout(descriptor_set_layout)
                    .build(device.clone())?;

                let right_desc_set = DescriptorPool::prepare_set(&right_desc_pool).allocate()?;

                right_desc_set.update(&[
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

    fn update_view_buffer(
        view_buffer: &Arc<Buffer<VRTransformations>>,
        transform: VRTransformations,
    ) -> VerboseResult<()> {
        let mut mapped = view_buffer.map_complete()?;
        mapped[0] = transform;

        Ok(())
    }

    fn update_plane_buffer(
        command_buffer: &Arc<CommandBuffer>,
        context: &Arc<Context>,
        plane_buffer: &Arc<Buffer<PlaneInfo>>,
        inverted_view: Matrix4<f32>,
        interpolation: &CPUInterpolation,
    ) -> VerboseResult<Option<(Vec<Arc<AccelerationStructure>>, Arc<AccelerationStructure>)>> {
        interpolation.calculate_interpolation(
            command_buffer,
            context,
            inverted_view,
            plane_buffer.map_complete()?,
        )
    }

    fn update_left_as(
        &self,
        blasses: Vec<Arc<AccelerationStructure>>,
        tlas: Arc<AccelerationStructure>,
    ) -> VerboseResult<()> {
        let mut acceleration_structures = self.acceleration_structures.lock()?;

        match acceleration_structures.as_mut() {
            Some(acceleration_structures) => {
                let (left, _) = acceleration_structures.stereo_mut()?;

                *left = Some((blasses, tlas));
            }
            None => {
                *acceleration_structures = Some(TargetMode::Stereo(None, Some((blasses, tlas))));
            }
        }

        Ok(())
    }

    fn update_right_as(
        &self,
        blasses: Vec<Arc<AccelerationStructure>>,
        tlas: Arc<AccelerationStructure>,
    ) -> VerboseResult<()> {
        let mut acceleration_structures = self.acceleration_structures.lock()?;

        match acceleration_structures.as_mut() {
            Some(acceleration_structures) => {
                let (_, right) = acceleration_structures.stereo_mut()?;

                *right = Some((blasses, tlas));
            }
            None => {
                *acceleration_structures = Some(TargetMode::Stereo(None, Some((blasses, tlas))));
            }
        }

        Ok(())
    }

    fn create_scene_data(
        context: &Arc<Context>,
        light_fields: Vec<LightField>,
    ) -> VerboseResult<(
        TargetMode<Arc<Buffer<PlaneInfo>>>,
        Vec<Arc<Image>>,
        CPUInterpolation,
    )> {
        let mut light_field_infos = Vec::with_capacity(light_fields.len());
        let mut images = Vec::new();

        let mut max_planes = 0;

        for light_field in light_fields.into_iter() {
            let frustum = light_field.frustum();
            let planes = light_field.into_data();

            let mut inner_planes = Vec::with_capacity(planes.len());

            for plane in planes.into_iter() {
                max_planes += 1;

                let mut image_infos = Vec::with_capacity(plane.content.len());

                // add plane contents to buffers
                for (image, ratios, center) in plane.content.into_iter() {
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
                    bary: vec4(0.0, 0.0, 0.0, 0.0),
                };

                // create vertex data
                let vertices = [
                    plane.left_bottom,
                    plane.left_top,
                    plane.right_bottom,
                    plane.right_bottom,
                    plane.left_top,
                    plane.right_top,
                ];

                inner_planes.push((plane_info, vertices, image_infos));
            }

            light_field_infos.push((inner_planes, frustum));
        }

        // --- create plane info buffer ---
        let plane_buffer = match context.render_core().images()? {
            TargetMode::Single(_) => TargetMode::Single(
                Buffer::builder()
                    .set_memory_properties(
                        VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                            | VK_MEMORY_PROPERTY_HOST_CACHED_BIT
                            | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT,
                    )
                    .set_usage(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT)
                    .set_size(max_planes)
                    .build(context.device().clone())?,
            ),
            TargetMode::Stereo(_, _) => TargetMode::Stereo(
                Buffer::builder()
                    .set_memory_properties(
                        VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                            | VK_MEMORY_PROPERTY_HOST_CACHED_BIT
                            | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT,
                    )
                    .set_usage(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT)
                    .set_size(max_planes)
                    .build(context.device().clone())?,
                Buffer::builder()
                    .set_memory_properties(
                        VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                            | VK_MEMORY_PROPERTY_HOST_CACHED_BIT
                            | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT,
                    )
                    .set_usage(VK_BUFFER_USAGE_STORAGE_BUFFER_BIT)
                    .set_size(max_planes)
                    .build(context.device().clone())?,
            ),
        };

        let command_buffer = context.render_core().allocate_primary_buffer()?;

        let interpolation =
            CPUInterpolation::new(context.queue(), &command_buffer, light_field_infos)?;

        Ok((plane_buffer, images, interpolation))
    }
}

#[derive(Debug, Clone)]
pub struct PlaneInfo {
    pub top_left: Vector4<f32>,
    pub top_right: Vector4<f32>,
    pub bottom_left: Vector4<f32>,
    pub bottom_right: Vector4<f32>,

    pub normal: Vector4<f32>,

    indices: Vector4<i32>,
    bary: Vector4<f32>,
}

impl PlaneInfo {
    pub fn clone(&self, indices: Vector4<i32>, bary: Vector2<f32>) -> Self {
        PlaneInfo {
            top_left: self.top_left,
            top_right: self.top_right,
            bottom_left: self.bottom_left,
            bottom_right: self.bottom_right,

            normal: self.normal,

            indices,
            bary: bary.extend(0.0).extend(0.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlaneImageInfo {
    pub ratios: PlaneImageRatios,
    pub center: Vector2<f32>,
    pub image_index: u32,
}
