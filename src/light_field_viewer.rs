use context::prelude::*;
use context::ContextObject;

use std::cell::RefCell;
use std::mem;
use std::ops::Deref;
use std::slice;
use std::sync::Arc;

use cgmath::{vec4, Deg};

use super::{example_object::ExampleVertex, light_field::LightField, view_emulator::ViewEmulator};

pub struct LightFieldViewer {
    context: Arc<Context>,

    render_targets: RefCell<TargetMode<RenderTarget>>,

    pipelines: RefCell<TargetMode<Arc<Pipeline>>>,

    view_buffers: TargetMode<Arc<Buffer<VRTransformations>>>,
    transform_descriptor: TargetMode<Arc<DescriptorSet>>,

    view_emulator: ViewEmulator,

    light_fields: Vec<LightField>,

    sample_count: VkSampleCountFlags,
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
            &[&light_field_desc_layout, desc],
        )?;

        Ok(Arc::new(LightFieldViewer {
            context: context.clone(),

            // config,
            render_targets: RefCell::new(render_targets),
            pipelines: RefCell::new(pipelines),

            view_buffers,
            transform_descriptor,

            view_emulator: ViewEmulator::new(context, Deg(10.0), 0.5),

            light_fields,

            sample_count,
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
                PresentationEventType::KeyDown(key) => self.view_emulator.on_key_down(key),
                PresentationEventType::KeyUp(key) => self.view_emulator.on_key_up(key),
                _ => (),
            }
        }

        Ok(())
    }
}

impl TScene for LightFieldViewer {
    fn update(&self) -> VerboseResult<()> {
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
        ) {
            (
                TargetMode::Single(index),
                TargetMode::Single(view_buffer),
                TargetMode::Single(example_descriptor),
                TargetMode::Single(pipeline),
                TargetMode::Single(render_target),
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
                )?;
            }
            (
                TargetMode::Stereo(left_index, right_index),
                TargetMode::Stereo(left_view_buffer, right_view_buffer),
                TargetMode::Stereo(left_descriptor, right_descriptor),
                TargetMode::Stereo(left_pipeline, right_pipeline),
                TargetMode::Stereo(left_render_target, right_render_target),
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
                )?;
            }
            _ => create_error!("invalid target mode setup"),
        }

        Ok(())
    }

    fn resize(&self) -> VerboseResult<()> {
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
            &[&light_field_desc_layout, desc],
        )?;

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

        let stages = [
            vertex_shader.pipeline_stage_info(),
            fragment_shader.pipeline_stage_info(),
        ];

        match render_targets {
            TargetMode::Single(render_target) => {
                let pipeline_layout =
                    PipelineLayout::new(context.device().clone(), descriptors, &[])?;

                let pipeline = Self::create_pipeline(
                    context,
                    &stages,
                    sample_count,
                    render_target.render_pass(),
                    &pipeline_layout,
                )?;

                Ok(TargetMode::Single(pipeline))
            }
            TargetMode::Stereo(left_render_target, right_render_target) => {
                let pipeline_layout =
                    PipelineLayout::new(context.device().clone(), descriptors, &[])?;

                let left_pipeline = Self::create_pipeline(
                    context,
                    &stages,
                    sample_count,
                    left_render_target.render_pass(),
                    &pipeline_layout,
                )?;

                let right_pipeline = Self::create_pipeline(
                    context,
                    &stages,
                    sample_count,
                    right_render_target.render_pass(),
                    &pipeline_layout,
                )?;

                Ok(TargetMode::Stereo(left_pipeline, right_pipeline))
            }
        }
    }

    fn create_pipeline(
        context: &Arc<Context>,
        stages: &[VkPipelineShaderStageCreateInfo],
        sample_count: VkSampleCountFlags,
        render_pass: &Arc<RenderPass>,
        pipeline_layout: &Arc<PipelineLayout>,
    ) -> VerboseResult<Arc<Pipeline>> {
        let input_bindings = [VkVertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<ExampleVertex>() as u32,
            inputRate: VK_VERTEX_INPUT_RATE_VERTEX,
        }];

        let input_attributes = [
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

        let vertex_input_state =
            VkPipelineVertexInputStateCreateInfo::new(0, &input_bindings, &input_attributes);

        let render_core = context.render_core();

        let viewport = VkViewport {
            x: 0.0,
            y: 0.0,
            width: render_core.width() as f32,
            height: render_core.height() as f32,
            minDepth: 0.0,
            maxDepth: 1.0,
        };
        let scissor = VkRect2D {
            offset: VkOffset2D { x: 0, y: 0 },
            extent: VkExtent2D {
                width: render_core.width(),
                height: render_core.height(),
            },
        };

        let viewport = VkPipelineViewportStateCreateInfo::new(
            VK_PIPELINE_VIEWPORT_STATE_CREATE_NULL_BIT,
            slice::from_ref(&viewport),
            slice::from_ref(&scissor),
        );

        let input_assembly = VkPipelineInputAssemblyStateCreateInfo::new(
            VK_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_NULL_BIT,
            VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST,
            false,
        );

        let multisample = VkPipelineMultisampleStateCreateInfo::new(
            VK_PIPELINE_MULTISAMPLE_STATE_CREATE_NULL_BIT,
            sample_count,
            false,
            0.0,
            &[],
            false,
            false,
        );

        let rasterization = VkPipelineRasterizationStateCreateInfo::new(
            VK_PIPELINE_RASTERIZATION_STATE_CREATE_NULL_BIT,
            false,
            false,
            VK_POLYGON_MODE_FILL,
            VK_CULL_MODE_NONE,
            VK_FRONT_FACE_COUNTER_CLOCKWISE,
            false,
            0.0,
            0.0,
            0.0,
            1.0,
        );

        let color_blend_attachment = VkPipelineColorBlendAttachmentState {
            blendEnable: VK_TRUE,
            srcColorBlendFactor: VK_BLEND_FACTOR_ONE,
            dstColorBlendFactor: VK_BLEND_FACTOR_ONE_MINUS_SRC_ALPHA,
            colorBlendOp: VK_BLEND_OP_ADD,
            srcAlphaBlendFactor: VK_BLEND_FACTOR_ONE,
            dstAlphaBlendFactor: VK_BLEND_FACTOR_ZERO,
            alphaBlendOp: VK_BLEND_OP_ADD,
            colorWriteMask: VK_COLOR_COMPONENT_R_BIT
                | VK_COLOR_COMPONENT_G_BIT
                | VK_COLOR_COMPONENT_B_BIT
                | VK_COLOR_COMPONENT_A_BIT,
        };

        let color_blend = VkPipelineColorBlendStateCreateInfo::new(
            VK_PIPELINE_COLOR_BLEND_STATE_CREATE_NULL_BIT,
            false,
            VK_LOGIC_OP_NO_OP,
            slice::from_ref(&color_blend_attachment),
            [1.0, 1.0, 1.0, 1.0],
        );

        let stencil_op_state = VkStencilOpState {
            failOp: VK_STENCIL_OP_KEEP,
            passOp: VK_STENCIL_OP_KEEP,
            depthFailOp: VK_STENCIL_OP_KEEP,
            compareOp: VK_COMPARE_OP_ALWAYS,
            compareMask: 0,
            writeMask: 0,
            reference: 0,
        };

        let depth_stencil = VkPipelineDepthStencilStateCreateInfo::new(
            VK_PIPELINE_DEPTH_STENCIL_STATE_CREATE_NULL_BIT,
            true,
            true,
            VK_COMPARE_OP_LESS,
            false,
            false,
            stencil_op_state.clone(),
            stencil_op_state,
            0.0,
            0.0,
        );

        let pipeline = Pipeline::new_graphics(
            context.device().clone(),
            None,
            0,
            &stages,
            Some(vertex_input_state),
            Some(input_assembly),
            None,
            Some(viewport),
            rasterization,
            Some(multisample),
            Some(depth_stencil),
            Some(color_blend),
            None,
            &pipeline_layout,
            render_pass,
            0,
            GraphicsPipelineExtensions {
                amd_rasterization_order: None,
            },
        )?;

        Ok(pipeline)
    }

    fn create_view_buffers(
        context: &Arc<Context>,
    ) -> VerboseResult<TargetMode<Arc<Buffer<VRTransformations>>>> {
        let render_core = context.render_core();

        match render_core.images() {
            TargetMode::Single(_) => Ok(TargetMode::Single(
                Buffer::new()
                    .set_usage(VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT)
                    .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
                    .set_size(1)
                    .build(context.device().clone())?,
            )),
            TargetMode::Stereo(_, _) => Ok(TargetMode::Stereo(
                Buffer::new()
                    .set_usage(VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT)
                    .set_memory_properties(VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)
                    .set_size(1)
                    .build(context.device().clone())?,
                Buffer::new()
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
        let descriptor_layout = DescriptorSetLayout::new()
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
        let descriptor_pool = DescriptorPool::new()
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
                    .set_prepared_targets(&images, 0, vec4(0.0, 0.0, 0.0, 1.0))
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
