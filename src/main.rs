use context::prelude::*;

mod config;
mod debug;
mod light_field;
mod light_field_viewer;
mod view_emulator;

use light_field::LightField;
use light_field_viewer::LightFieldViewer;

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

    let (light_field_1, mut frustum_1) = LightField::new(&context, "data/shot_01")?;
    let (light_field_2, mut frustum_2) = LightField::new(&context, "data/shot_02")?;
    let (light_field_3, mut frustum_3) = LightField::new(&context, "data/shot_03")?;

    let mut frustum = Vec::new();
    frustum.append(&mut frustum_1);
    frustum.append(&mut frustum_2);
    frustum.append(&mut frustum_3);

    let light_field_viewer = LightFieldViewer::new(
        &context,
        sample_count,
        vec![light_field_1, light_field_2, light_field_3],
        frustum,
    )?;

    context.set_context_object(Some(light_field_viewer.clone()))?;
    context.render_core().add_scene(light_field_viewer)?;

    context.run()?;

    Ok(())
}
