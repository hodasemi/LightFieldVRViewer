use context::prelude::*;

use super::light_field_frustum::LightFieldFrustum;

use std::collections::HashMap;
use std::sync::Arc;

pub struct LightFieldRenderer {}

impl LightFieldRenderer {
    pub fn new(
        frustums: Vec<LightFieldFrustum>,
        image_data: Vec<(Vec<(Arc<Image>, f32)>, usize, usize)>,
    ) -> VerboseResult<LightFieldRenderer> {
        let mut sorted_frustums = HashMap::new();

        for frustum in frustums.iter() {
            sorted_frustums.insert(frustum.position(), frustum);
        }

        for (images, x, y) in image_data.iter() {
            let frustum = sorted_frustums
                .get(&(*x, *y))
                .ok_or(format!("no frustum found at ({}, {})", x, y))?;
        }

        Ok(LightFieldRenderer {})
    }

    pub fn render(
        &self,
        command_buffer: &Arc<CommandBuffer>,
        transform_descriptor: &Arc<DescriptorSet>,
    ) -> VerboseResult<()> {
        Ok(())
    }
}
