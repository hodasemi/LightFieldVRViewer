use context::prelude::*;

mod config;
mod example_object;
mod light_field_viewer;

use light_field_viewer::LightFieldViewer;

fn main() -> VerboseResult<()> {
    let sample_count = VK_SAMPLE_COUNT_1_BIT;

    let context = Context::new()
        .set_vulkan_debug_info(VulkanDebugInfo {
            debugging: false,
            renderdoc: false,
            steam_layer: false,
            use_util: false,
            verbose: false,
        })
        .enable_vsync()
        .set_sample_count(sample_count)
        .set_vr_mode(VRMode::OpenVR)
        // .set_openxr_json("/usr/share/openxr/1/openxr_monado.json")
        .build()?;

    let light_field_viewer = LightFieldViewer::new(&context, sample_count)?;

    context.set_game_object(Some(light_field_viewer.clone()))?;
    context
        .render_core()
        .add_scene(light_field_viewer.clone())?;

    context.run()?;

    Ok(())
}
