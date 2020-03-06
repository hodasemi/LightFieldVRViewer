use cgmath::{vec4, Matrix4, Vector2, Vector3, Vector4};
use context::prelude::*;

use std::sync::Arc;

/// Vertex structure used as input type for feet pass
#[derive(Debug, Clone)]
pub struct TexturedVertex {
    pub position: Vector3<f32>,
    pub uv: Vector2<f32>,
}

/// Vertex structure used as input type for outline pass
#[derive(Debug, Clone)]
pub struct ColoredVertex {
    pub position: Vector3<f32>,
    pub color: Vector4<f32>,
}

/// Keeps track of pipelines and render passes
pub struct Rasterizer {
    triangle_pipelines: TargetMode<Arc<Pipeline>>,
    line_pipelines: TargetMode<Arc<Pipeline>>,
    render_targets: TargetMode<RenderTarget>,
}

impl Rasterizer {
    /// Creates `Rasterizer`
    ///
    /// # Arguments
    ///
    /// * `context` Context handle
    pub fn new(context: &Arc<Context>) -> VerboseResult<Self> {
        let render_targets = Self::create_render_targets(context)?;
        let triangle_pipelines = Self::create_triangle_pipelines(context, &render_targets)?;
        let line_pipelines = Self::create_line_pipelines(context, &render_targets)?;

        Ok(Rasterizer {
            triangle_pipelines,
            line_pipelines,
            render_targets,
        })
    }

    /// Returns pipeline, used by feet
    pub fn triangle_pipelines(&self) -> &TargetMode<Arc<Pipeline>> {
        &self.triangle_pipelines
    }

    /// Returns pipeline, used by outlines
    pub fn line_pipelines(&self) -> &TargetMode<Arc<Pipeline>> {
        &self.line_pipelines
    }

    /// Returns `RenderTarget`, wrapper type for RenderPass and Framebuffer
    pub fn render_targets(&self) -> &TargetMode<RenderTarget> {
        &self.render_targets
    }

    fn create_line_pipelines(
        context: &Arc<Context>,
        render_targets: &TargetMode<RenderTarget>,
    ) -> VerboseResult<TargetMode<Arc<Pipeline>>> {
        let pipeline_layout = PipelineLayout::builder()
            .add_push_constant(VkPushConstantRange::new(
                VK_SHADER_STAGE_VERTEX_BIT,
                0,
                2 * std::mem::size_of::<Matrix4<f32>>() as u32,
            ))
            .build(context.device().clone())?;

        match render_targets {
            TargetMode::Single(render_target) => {
                Ok(TargetMode::Single(Self::create_line_pipeline(
                    context.device(),
                    &pipeline_layout,
                    render_target.render_pass(),
                    0,
                    render_target.width(),
                    render_target.height(),
                )?))
            }
            TargetMode::Stereo(left_render_target, right_render_target) => Ok(TargetMode::Stereo(
                Self::create_line_pipeline(
                    context.device(),
                    &pipeline_layout,
                    left_render_target.render_pass(),
                    0,
                    left_render_target.width(),
                    left_render_target.height(),
                )?,
                Self::create_line_pipeline(
                    context.device(),
                    &pipeline_layout,
                    right_render_target.render_pass(),
                    0,
                    right_render_target.width(),
                    right_render_target.height(),
                )?,
            )),
        }
    }

    fn create_triangle_pipelines(
        context: &Arc<Context>,
        render_targets: &TargetMode<RenderTarget>,
    ) -> VerboseResult<TargetMode<Arc<Pipeline>>> {
        let set_layout = DescriptorSetLayout::builder()
            .add_layout_binding(
                0,
                VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER,
                VK_SHADER_STAGE_FRAGMENT_BIT,
                0,
            )
            .build(context.device().clone())?;

        let pipeline_layout = PipelineLayout::builder()
            .add_descriptor_set_layout(&set_layout)
            .add_push_constant(VkPushConstantRange::new(
                VK_SHADER_STAGE_VERTEX_BIT,
                0,
                2 * std::mem::size_of::<Matrix4<f32>>() as u32,
            ))
            .build(context.device().clone())?;

        match render_targets {
            TargetMode::Single(render_target) => {
                Ok(TargetMode::Single(Self::create_triangle_pipeline(
                    context.device(),
                    &pipeline_layout,
                    render_target.render_pass(),
                    0,
                    render_target.width(),
                    render_target.height(),
                )?))
            }
            TargetMode::Stereo(left_render_target, right_render_target) => Ok(TargetMode::Stereo(
                Self::create_triangle_pipeline(
                    context.device(),
                    &pipeline_layout,
                    left_render_target.render_pass(),
                    0,
                    left_render_target.width(),
                    left_render_target.height(),
                )?,
                Self::create_triangle_pipeline(
                    context.device(),
                    &pipeline_layout,
                    right_render_target.render_pass(),
                    0,
                    right_render_target.width(),
                    right_render_target.height(),
                )?,
            )),
        }
    }

    fn create_render_targets(context: &Arc<Context>) -> VerboseResult<TargetMode<RenderTarget>> {
        match context.render_core().images()? {
            TargetMode::Single(images) => Ok(TargetMode::Single(Self::create_render_target(
                context, &images,
            )?)),
            TargetMode::Stereo(left_images, right_images) => Ok(TargetMode::Stereo(
                Self::create_render_target(context, &left_images)?,
                Self::create_render_target(context, &right_images)?,
            )),
        }
    }

    fn create_line_pipeline(
        device: &Arc<Device>,
        pipeline_layout: &Arc<PipelineLayout>,
        render_pass: &Arc<RenderPass>,
        subpass: u32,
        width: u32,
        height: u32,
    ) -> VerboseResult<Arc<Pipeline>> {
        let vertex_shader_text = include_bytes!("../shader/line.vert.spv");
        let fragment_shader_text = include_bytes!("../shader/line.frag.spv");

        let input_bindings = vec![VkVertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<ColoredVertex>() as u32,
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
            // color
            VkVertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: VK_FORMAT_R32G32B32A32_SFLOAT,
                offset: 12,
            },
        ];

        Pipeline::new_graphics()
            .set_vertex_shader(
                ShaderModule::from_slice(device.clone(), vertex_shader_text, ShaderType::Vertex)?,
                input_bindings,
                input_attributes,
            )
            .set_fragment_shader(ShaderModule::from_slice(
                device.clone(),
                fragment_shader_text,
                ShaderType::Fragment,
            )?)
            .input_assembly(VK_PRIMITIVE_TOPOLOGY_LINE_LIST, false)
            .default_depth_stencil(false, false)
            .default_color_blend(vec![VkPipelineColorBlendAttachmentState::default()])
            .default_rasterization(VK_CULL_MODE_NONE, VK_FRONT_FACE_COUNTER_CLOCKWISE)
            .default_multisample(VK_SAMPLE_COUNT_1_BIT)
            .add_viewport(VkViewport {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: height as f32,
                minDepth: 0.0,
                maxDepth: 1.0,
            })
            .add_scissor(VkRect2D {
                offset: VkOffset2D { x: 0, y: 0 },
                extent: VkExtent2D { width, height },
            })
            .build(device.clone(), pipeline_layout, render_pass, subpass)
    }

    fn create_triangle_pipeline(
        device: &Arc<Device>,
        pipeline_layout: &Arc<PipelineLayout>,
        render_pass: &Arc<RenderPass>,
        subpass: u32,
        width: u32,
        height: u32,
    ) -> VerboseResult<Arc<Pipeline>> {
        let vertex_shader_text = include_bytes!("../shader/feet.vert.spv");
        let fragment_shader_text = include_bytes!("../shader/feet.frag.spv");

        let input_bindings = vec![VkVertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<TexturedVertex>() as u32,
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
            // uv
            VkVertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: VK_FORMAT_R32G32_SFLOAT,
                offset: 12,
            },
        ];

        Pipeline::new_graphics()
            .set_vertex_shader(
                ShaderModule::from_slice(device.clone(), vertex_shader_text, ShaderType::Vertex)?,
                input_bindings,
                input_attributes,
            )
            .set_fragment_shader(ShaderModule::from_slice(
                device.clone(),
                fragment_shader_text,
                ShaderType::Fragment,
            )?)
            .input_assembly(VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST, false)
            .default_depth_stencil(false, false)
            .default_color_blend(vec![VkPipelineColorBlendAttachmentState::default()])
            .default_rasterization(VK_CULL_MODE_NONE, VK_FRONT_FACE_COUNTER_CLOCKWISE)
            .default_multisample(VK_SAMPLE_COUNT_1_BIT)
            .add_viewport(VkViewport {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: height as f32,
                minDepth: 0.0,
                maxDepth: 1.0,
            })
            .add_scissor(VkRect2D {
                offset: VkOffset2D { x: 0, y: 0 },
                extent: VkExtent2D { width, height },
            })
            .build(device.clone(), pipeline_layout, render_pass, subpass)
    }

    fn create_render_target(
        context: &Arc<Context>,
        images: &[Arc<Image>],
    ) -> VerboseResult<RenderTarget> {
        let render_core = context.render_core();

        let width = render_core.width();
        let height = render_core.height();

        RenderTarget::new(width, height)
            .set_prepared_targets(images, 0, vec4(0.0, 0.0, 0.0, 0.0), false)
            // .add_target_info(CustomTarget::depth())
            .build(context.device(), context.queue())
    }
}
