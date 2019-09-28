use context::prelude::*;

fn main() -> VerboseResult<()> {
    let context = Context::new()
        .set_vulkan_debug_info(VulkanDebugInfo {
            debugging: true,
            renderdoc: true,
            steam_layer: false,
            use_util: false,
            verbose: false,
        })
        .set_vr_mode(VRMode::OpenXR)
        .set_openxr_json("/usr/share/openxr/1/openxr_monado.json")
        .build()?;

    context.run()?;

    Ok(())
}
