use context::prelude::*;
use context::ContextObject;

use std::cell::{Cell, RefCell};
use std::mem;
use std::ops::Deref;
use std::sync::Arc;

use cgmath::{vec3, vec4, Deg, Vector3};

use super::debug::coordinate_system::CoordinateSystem;
use super::{example_object::ExampleVertex, light_field::LightField, view_emulator::ViewEmulator};

pub const DEFAULT_FORWARD: Vector3<f32> = vec3(0.0, 0.0, -1.0);
pub const UP: Vector3<f32> = vec3(0.0, 1.0, 0.0);

pub struct LightFieldViewer {
    context: Arc<Context>,

    render_targets: RefCell<TargetMode<RenderTarget>>,

    pipelines: RefCell<TargetMode<Arc<Pipeline>>>,

    view_buffers: TargetMode<Arc<Buffer<VRTransformations>>>,
    transform_descriptor: TargetMode<Arc<DescriptorSet>>,

    view_emulator: ViewEmulator,

    light_fields: Vec<LightField>,

    sample_count: VkSampleCountFlags,

    coordinate_systems: RefCell<TargetMode<CoordinateSystem>>,

    last_time_stemp: Cell<f64>,
    fps_count: Cell<u32>,
}

impl LightFieldViewer {
    pub fn new(
        context: &Arc<Context>,
        sample_count: VkSampleCountFlags,
        light_fields: Vec<LightField>,
    ) -> VerboseResult<Arc<Self>> {
        let view_buffers = Self::create_view_buffers(context)?;

        let transform_descriptor = Self::create_transform_descriptor(context, &view_buffers)?;
        let light_field_desc_layout = LightField::descriptor_layout(context.device())?;

        let desc = match &transform_descriptor {
            TargetMode::Single(desc) => desc,
            TargetMode::Stereo(desc, _) => desc,
        };

        let render_targets = Self::create_render_targets(context)?;
        let pipelines = Self::create_pipelines(
            context,
            "shader/quad.vert.spv",
            "shader/quad.frag.spv",
            sample_count,
            &render_targets,
            &[desc, &light_field_desc_layout],
        )?;

        let coordinate_systems =
            Self::create_coordinate_systems(context, &render_targets, sample_count, desc)?;

        Ok(Arc::new(LightFieldViewer {
            context: context.clone(),

            // config,
            render_targets: RefCell::new(render_targets),
            pipelines: RefCell::new(pipelines),

            view_buffers,
            transform_descriptor,

            view_emulator: ViewEmulator::new(context, Deg(45.0), 2.5),

            light_fields,

            sample_count,

            coordinate_systems: RefCell::new(coordinate_systems),

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
            self.pipelines.try_borrow()?.deref(),
            self.render_targets.try_borrow()?.deref(),
            self.coordinate_systems.try_borrow()?.deref(),
        ) {
            (
                TargetMode::Single(index),
                TargetMode::Single(view_buffer),
                TargetMode::Single(example_descriptor),
                TargetMode::Single(pipeline),
                TargetMode::Single(render_target),
                TargetMode::Single(coordinate_system),
            ) => {
                Self::render(
                    *index,
                    render_target,
                    pipeline,
                    command_buffer,
                    view_buffer,
                    &self.view_emulator.simulation_transform(),
                    example_descriptor,
                    &self.light_fields,
                    coordinate_system,
                )?;
            }
            (
                TargetMode::Stereo(left_index, right_index),
                TargetMode::Stereo(left_view_buffer, right_view_buffer),
                TargetMode::Stereo(left_descriptor, right_descriptor),
                TargetMode::Stereo(left_pipeline, right_pipeline),
                TargetMode::Stereo(left_render_target, right_render_target),
                TargetMode::Stereo(left_coordinate_system, right_coordinate_system),
            ) => {
                let (left_transform, right_transform) = transforms
                    .as_ref()
                    .ok_or("no transforms present")?
                    .stereo()?;

                Self::render(
                    *left_index,
                    left_render_target,
                    left_pipeline,
                    command_buffer,
                    left_view_buffer,
                    left_transform,
                    left_descriptor,
                    &self.light_fields,
                    left_coordinate_system,
                )?;

                Self::render(
                    *right_index,
                    right_render_target,
                    right_pipeline,
                    command_buffer,
                    right_view_buffer,
                    right_transform,
                    right_descriptor,
                    &self.light_fields,
                    right_coordinate_system,
                )?;
            }
            _ => create_error!("invalid target mode setup"),
        }

        Ok(())
    }

    fn resize(&self) -> VerboseResult<()> {
        self.view_emulator.on_resize();

        let render_targets = Self::create_render_targets(&self.context)?;

        let light_field_desc_layout = LightField::descriptor_layout(self.context.device())?;

        let desc = match &self.transform_descriptor {
            TargetMode::Single(desc) => desc,
            TargetMode::Stereo(desc, _) => desc,
        };

        let pipelines = Self::create_pipelines(
            &self.context,
            "shader/quad.vert.spv",
            "shader/quad.frag.spv",
            self.sample_count,
            &render_targets,
            &[desc, &light_field_desc_layout],
        )?;

        let coordinate_systems = Self::create_coordinate_systems(
            &self.context,
            &render_targets,
            self.sample_count,
            desc,
        )?;

        *self.coordinate_systems.try_borrow_mut()? = coordinate_systems;
        *self.render_targets.try_borrow_mut()? = render_targets;
        *self.pipelines.try_borrow_mut()? = pipelines;

        Ok(())
    }
}

impl LightFieldViewer {
    fn render(
        index: usize,
        render_target: &RenderTarget,
        pipeline: &Arc<Pipeline>,
        command_buffer: &Arc<CommandBuffer>,
        view_buffer: &Arc<Buffer<VRTransformations>>,
        transform: &VRTransformations,
        descriptor_set: &Arc<DescriptorSet>,
        light_fields: &[LightField],
        coordinate_system: &CoordinateSystem,
    ) -> VerboseResult<()> {
        {
            let mut mapped = view_buffer.map_complete()?;
            mapped[0] = *transform;
        }

        render_target.begin(command_buffer, VK_SUBPASS_CONTENTS_INLINE, index);

        command_buffer.bind_pipeline(pipeline)?;

        for light_field in light_fields {
            light_field.render(command_buffer, descriptor_set)?;
        }

        coordinate_system.render(command_buffer, descriptor_set)?;

        render_target.end(command_buffer);

        Ok(())
    }

    fn create_pipelines(
        context: &Arc<Context>,
        vs: &str,
        fs: &str,
        sample_count: VkSampleCountFlags,
        render_targets: &TargetMode<RenderTarget>,
        descriptors: &[&dyn VkHandle<VkDescriptorSetLayout>],
    ) -> VerboseResult<TargetMode<Arc<Pipeline>>> {
        let vertex_shader = ShaderModule::new(context.device().clone(), vs, ShaderType::Vertex)?;
        let fragment_shader =
            ShaderModule::new(context.device().clone(), fs, ShaderType::Fragment)?;

        match render_targets {
            TargetMode::Single(render_target) => {
                let pipeline_layout =
                    PipelineLayout::new(context.device().clone(), descriptors, &[])?;

                let pipeline = Self::create_pipeline(
                    context,
                    sample_count,
                    render_target.render_pass(),
                    &pipeline_layout,
                    vertex_shader,
                    fragment_shader,
                )?;

                Ok(TargetMode::Single(pipeline))
            }
            TargetMode::Stereo(left_render_target, right_render_target) => {
                let pipeline_layout =
                    PipelineLayout::new(context.device().clone(), descriptors, &[])?;

                let left_pipeline = Self::create_pipeline(
                    context,
                    sample_count,
                    left_render_target.render_pass(),
                    &pipeline_layout,
                    vertex_shader.clone(),
                    fragment_shader.clone(),
                )?;

                let right_pipeline = Self::create_pipeline(
                    context,
                    sample_count,
                    right_render_target.render_pass(),
                    &pipeline_layout,
                    vertex_shader,
                    fragment_shader,
                )?;

                Ok(TargetMode::Stereo(left_pipeline, right_pipeline))
            }
        }
    }

    fn create_pipeline(
        context: &Arc<Context>,
        sample_count: VkSampleCountFlags,
        render_pass: &Arc<RenderPass>,
        pipeline_layout: &Arc<PipelineLayout>,
        vertex_shader: Arc<ShaderModule>,
        fragment_shader: Arc<ShaderModule>,
    ) -> VerboseResult<Arc<Pipeline>> {
        let render_core = context.render_core();

        let input_bindings = vec![VkVertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<ExampleVertex>() as u32,
            inputRate: VK_VERTEX_INPUT_RATE_VERTEX,
        }];

        let input_attributes = vec![
            // position
            VkVertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: VK_FORMAT_R32G32B32_SFLOAT,
                offset: 0,
            },
            // uvs
            VkVertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: VK_FORMAT_R32G32_SFLOAT,
                offset: 12,
            },
        ];

        Pipeline::new_graphics()
            .set_vertex_shader(vertex_shader, input_bindings, input_attributes)
            .set_fragment_shader(fragment_shader)
            .add_viewport(VkViewport {
                x: 0.0,
                y: 0.0,
                width: render_core.width() as f32,
                height: render_core.height() as f32,
                minDepth: 0.0,
                maxDepth: 1.0,
            })
            .add_scissor(VkRect2D {
                offset: VkOffset2D { x: 0, y: 0 },
                extent: VkExtent2D {
                    width: render_core.width(),
                    height: render_core.height(),
                },
            })
            .default_multisample(sample_count)
            .default_rasterization(VK_CULL_MODE_NONE, VK_FRONT_FACE_COUNTER_CLOCKWISE)
            .default_color_blend(vec![VkPipelineColorBlendAttachmentState::default()])
            .default_depth_stencil(true, false)
            .input_assembly(VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST, false)
            .build(context.device().clone(), pipeline_layout, render_pass, 0)
    }

    fn create_coordinate_systems(
        context: &Arc<Context>,
        render_targets: &TargetMode<RenderTarget>,
        sample_count: VkSampleCountFlags,
        descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<TargetMode<CoordinateSystem>> {
        match render_targets {
            TargetMode::Single(render_target) => Ok(TargetMode::Single(CoordinateSystem::new(
                context,
                sample_count,
                render_target.render_pass(),
                descriptor,
            )?)),
            TargetMode::Stereo(left_render_target, right_render_target) => {
                let left_cs = CoordinateSystem::new(
                    context,
                    sample_count,
                    left_render_target.render_pass(),
                    descriptor,
                )?;
                let right_cs = CoordinateSystem::new(
                    context,
                    sample_count,
                    right_render_target.render_pass(),
                    descriptor,
                )?;

                Ok(TargetMode::Stereo(left_cs, right_cs))
            }
        }
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
                VK_SHADER_STAGE_VERTEX_BIT,
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

    fn create_render_targets(context: &Arc<Context>) -> VerboseResult<TargetMode<RenderTarget>> {
        let render_core = context.render_core();
        let images = render_core.images();

        match images {
            TargetMode::Single(images) => {
                let render_target = RenderTarget::new(render_core.width(), render_core.height())
                    .set_prepared_targets(&images, 0, vec4(0.2, 0.2, 0.2, 1.0))
                    .add_target_info(CustomTarget::depth())
                    .build(context.device(), context.queue())?;

                Ok(TargetMode::Single(render_target))
            }
            TargetMode::Stereo(left_images, right_images) => {
                let left_render_target =
                    RenderTarget::new(render_core.width(), render_core.height())
                        .set_prepared_targets(&left_images, 0, vec4(0.2, 0.2, 0.2, 1.0))
                        .add_target_info(CustomTarget::depth())
                        .build(context.device(), context.queue())?;

                let right_render_target =
                    RenderTarget::new(render_core.width(), render_core.height())
                        .set_prepared_targets(&right_images, 0, vec4(0.2, 0.2, 0.2, 1.0))
                        .add_target_info(CustomTarget::depth())
                        .build(context.device(), context.queue())?;

                Ok(TargetMode::Stereo(left_render_target, right_render_target))
            }
        }
    }
}
