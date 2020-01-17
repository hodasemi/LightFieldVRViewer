use context::prelude::*;

use std::sync::Arc;

pub struct Rasterizer {
    pipelines: TargetMode<Arc<Pipeline>>,
    render_targets: TargetMode<RenderTarget>,
}

impl Rasterizer {
    pub fn new(context: &Arc<Context>) -> VerboseResult<Self> {
        let render_targets = Self::create_render_targets()?;
        let pipelines = Self::create_pipelines()?;

        Ok(Rasterizer {
            pipelines,
            render_targets,
        })
    }

    fn create_pipelines() -> VerboseResult<TargetMode<Arc<Pipeline>>> {
        todo!()
    }

    fn create_render_targets() -> VerboseResult<TargetMode<RenderTarget>> {
        todo!()
    }

    fn create_pipeline(
        device: &Arc<Device>,
        pipeline_layout: &Arc<PipelineLayout>,
        render_pass: &Arc<RenderPass>,
        subpass: u32,
    ) -> VerboseResult<Arc<Pipeline>> {
        Pipeline::new_graphics().set_vertex_shader



        build(device.clone(), pipeline_layout, render_pass, subpass)
    }

    fn create_render_target() -> VerboseResult<RenderTarget> {
        todo!()
    }
}
