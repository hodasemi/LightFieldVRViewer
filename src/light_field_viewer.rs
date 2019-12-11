use context::prelude::*;
use context::ContextObject;

use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::sync::Arc;

use cgmath::{vec3, vec4, Deg, Vector3};

use super::debug::{coordinate_system::CoordinateSystem, frustums::FrustumRenderer};
use super::{
    light_field::{
        light_field_data::LightFieldData, light_field_frustum::LightFieldFrustum, LightField,
    },
    view_emulator::ViewEmulator,
};

pub const DEFAULT_FORWARD: Vector3<f32> = vec3(0.0, 0.0, -1.0);
pub const UP: Vector3<f32> = vec3(0.0, 1.0, 0.0);

pub struct LightFieldViewer {
    context: Arc<Context>,

    view_buffers: TargetMode<Arc<Buffer<VRTransformations>>>,
    transform_descriptor: TargetMode<Arc<DescriptorSet>>,

    ray_tracing_pipeline: Arc<Pipeline>,
    sbt: ShaderBindingTable,

    // blas: Arc<AccelerationStructure>,
    // tlas: Arc<AccelerationStructure>,
    view_emulator: ViewEmulator,

    last_time_stemp: Cell<f64>,
    fps_count: Cell<u32>,
}

impl LightFieldViewer {
    pub fn new(context: &Arc<Context>, light_fields: Vec<LightField>) -> VerboseResult<Arc<Self>> {
        let view_buffers = Self::create_view_buffers(context)?;

        let transform_descriptor = Self::create_transform_descriptor(context, &view_buffers)?;
        // let light_field_desc_layout =
        //     LightFieldRenderer::descriptor_layout(context.device().clone())?;

        let desc = match &transform_descriptor {
            TargetMode::Single(desc) => desc,
            TargetMode::Stereo(desc, _) => desc,
        };

        let device = context.device();

        let first_desc_layout = DescriptorSetLayout::builder()
            .add_layout_binding(
                0,
                VK_DESCRIPTOR_TYPE_ACCELERATION_STRUCTURE_NV,
                VK_SHADER_STAGE_RAYGEN_BIT_NV | VK_SHADER_STAGE_CLOSEST_HIT_BIT_NV,
                0,
            )
            .add_layout_binding(
                1,
                VK_DESCRIPTOR_TYPE_STORAGE_IMAGE,
                VK_SHADER_STAGE_RAYGEN_BIT_NV,
                0,
            )
            .build(device.clone())?;

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
            .build(device, &[&first_desc_layout, desc])?;

        Ok(Arc::new(LightFieldViewer {
            context: context.clone(),
            view_buffers,
            transform_descriptor,

            ray_tracing_pipeline: pipeline,
            sbt,

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
        // match (indices, &self.view_buffers, &self.transform_descriptor) {
        //     (
        //         TargetMode::Single(index),
        //         TargetMode::Single(view_buffer),
        //         TargetMode::Single(example_descriptor),
        //     ) => {
        //         Self::render(
        //             *index,
        //             render_target,
        //             pipeline,
        //             command_buffer,
        //             view_buffer,
        //             &self.view_emulator.simulation_transform(),
        //             example_descriptor,
        //             &self.light_fields,
        //             coordinate_system,
        //             frustum_renderer,
        //         )?;
        //     }
        //     (
        //         TargetMode::Stereo(left_index, right_index),
        //         TargetMode::Stereo(left_view_buffer, right_view_buffer),
        //         TargetMode::Stereo(left_descriptor, right_descriptor),
        //     ) => {
        //         let (left_transform, right_transform) = transforms
        //             .as_ref()
        //             .ok_or("no transforms present")?
        //             .stereo()?;

        //         Self::render(
        //             *left_index,
        //             left_render_target,
        //             left_pipeline,
        //             command_buffer,
        //             left_view_buffer,
        //             left_transform,
        //             left_descriptor,
        //             &self.light_fields,
        //             left_coordinate_system,
        //             left_frustum_renderer,
        //         )?;

        //         Self::render(
        //             *right_index,
        //             right_render_target,
        //             right_pipeline,
        //             command_buffer,
        //             right_view_buffer,
        //             right_transform,
        //             right_descriptor,
        //             &self.light_fields,
        //             right_coordinate_system,
        //             right_frustum_renderer,
        //         )?;
        //     }
        //     _ => create_error!("invalid target mode setup"),
        // }

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
        descriptor_set: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        // update
        {
            let mut mapped = view_buffer.map_complete()?;
            mapped[0] = *transform;
        }

        command_buffer.bind_pipeline(&self.ray_tracing_pipeline)?;
        // command_buffer.bind_descriptor_sets_minimal(&descs)?;
        // command_buffer.trace_rays_sbt(&self.sbt, self.width, self.height, 1);

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
}
