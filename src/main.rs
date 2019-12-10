use context::prelude::*;

mod config;
mod debug;
mod light_field;
mod light_field_viewer;
mod view_emulator;

use light_field::{light_field_frustum::LightFieldFrustum, LightField};
use light_field_viewer::LightFieldViewer;

use std::thread;

fn main() -> VerboseResult<()> {
    let sample_count = VK_SAMPLE_COUNT_1_BIT;

    let context = Context::new()
        .set_vulkan_debug_info(VulkanDebugInfo {
            debugging: true,
            renderdoc: false,
            steam_layer: false,
            use_util: false,
            verbose: false,
        })
        .set_window_info(WindowCreateInfo {
            title: "Light Field Desktop Viewer".to_string(),
            width: 1280,
            height: 720,
            fullscreen: false,
            requested_display: None,
        })
        // .enable_vsync()
        .enable_keyboard()
        .set_sample_count(sample_count)
        // .set_vr_mode(VRMode::OpenVR)
        // .set_openxr_json("/usr/share/openxr/1/openxr_monado.json")
        .build()?;

    let data = ["data/shot_01", "data/shot_02", "data/shot_03"];

    let mut join_handles: Vec<
        thread::JoinHandle<VerboseResult<(LightField, Vec<LightFieldFrustum>)>>,
    > = data
        .iter()
        .cloned()
        .map(|field_path| {
            let context_clone = context.clone();

            thread::spawn(
                move || -> VerboseResult<(LightField, Vec<LightFieldFrustum>)> {
                    LightField::new(&context_clone, field_path)
                },
            )
        })
        .collect();

    let mut frustums = Vec::new();
    let mut light_fields = Vec::new();

    while let Some(join_handle) = join_handles.pop() {
        let (light_field, mut frustum) = join_handle.join()??;

        frustums.append(&mut frustum);
        light_fields.push(light_field);
    }

    let light_field_viewer = LightFieldViewer::new(&context, sample_count, light_fields, frustums)?;

    context.set_context_object(Some(light_field_viewer.clone()))?;
    context.render_core().add_scene(light_field_viewer)?;

    context.run()?;

    Ok(())
}
