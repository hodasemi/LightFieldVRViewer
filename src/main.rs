use cgmath::Deg;
use context::prelude::*;

mod config;
mod interpolation;
mod light_field;
mod light_field_viewer;
mod rasterizer;
mod view_emulator;

use light_field::LightField;
use light_field_viewer::LightFieldViewer;

use std::sync::Arc;
use std::thread;

fn main() -> VerboseResult<()> {
    let viewer_config = VrViewerConfig::load("settings.conf")?;

    let context = match create_vr_context() {
        Ok(context) => context,
        Err(msg) => {
            println!("{:?}", msg);
            println!("failed creating VR Context");

            create_desktop_context(viewer_config.enable_vsync)?
        }
    };

    // let context = create_desktop_context(viewer_config.enable_vsync)?;

    // spawn threads to load light fields
    let mut join_handles: Vec<thread::JoinHandle<VerboseResult<LightField>>> = viewer_config
        .light_fields
        .iter()
        .cloned()
        .map(|field_path| {
            let context_clone = context.clone();

            thread::spawn(move || LightField::new(&context_clone, &field_path))
        })
        .collect();

    // wait for thread to join
    let mut light_fields = Vec::new();

    while let Some(join_handle) = join_handles.pop() {
        let light_field = join_handle.join()??;

        light_fields.push(light_field);
    }

    // create viewer
    let light_field_viewer = LightFieldViewer::new(
        &context,
        light_fields,
        viewer_config.rotation_speed,
        viewer_config.movement_speed,
    )?;

    println!("created viewer!");

    // add viewer to context
    context.set_context_object(Some(light_field_viewer.clone()))?;
    context.render_core().add_scene(light_field_viewer)?;

    // loop
    context.run()?;

    Ok(())
}

fn create_vr_context() -> VerboseResult<Arc<Context>> {
    Context::new()
        .set_vulkan_debug_info(VulkanDebugInfo {
            debugging: false,
            renderdoc: false,
            steam_layer: false,
            use_util: false,
            verbose: false,
        })
        .set_vr_mode(VRMode::OpenVR)
        .build()
}

fn create_desktop_context(enable_vsync: bool) -> VerboseResult<Arc<Context>> {
    let mut context_builder = Context::new()
        .set_vulkan_debug_info(VulkanDebugInfo {
            debugging: false,
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
        .enable_keyboard();

    if enable_vsync {
        context_builder = context_builder.enable_vsync();
    }

    context_builder.build()
}

const DESKTOP_META: &str = "Desktop";
const INFO_META: &str = "Info";

const MOVEMENT_SPEED: &str = "movement_speed";
const ROTATION_SPEED: &str = "rotation_speed";
const VSYNC: &str = "enable_vsync";
const LIGHT_FIELDS: &str = "light_fields";

struct VrViewerConfig {
    // in meter per second
    movement_speed: f32,

    // in degrees per second
    rotation_speed: Deg<f32>,

    // only in desktop mode
    enable_vsync: bool,

    light_fields: Vec<String>,
}

impl VrViewerConfig {
    fn load(file: &str) -> VerboseResult<VrViewerConfig> {
        let mut config = VrViewerConfig::default();

        let config_data = ConfigHandler::read_config(file)?;

        if let Some(info) = config_data.get(DESKTOP_META) {
            if let Some(value) = info.get(MOVEMENT_SPEED) {
                config.movement_speed = value.to_value()?;
            }

            if let Some(value) = info.get(ROTATION_SPEED) {
                config.rotation_speed = Deg(value.to_value()?);
            }

            if let Some(value) = info.get(VSYNC) {
                config.enable_vsync = value.to_value()?;
            }
        }

        if let Some(info) = config_data.get(INFO_META) {
            if let Some(value) = info.get(LIGHT_FIELDS) {
                config.light_fields = value.to_array()?;
            }
        }

        Ok(config)
    }
}

impl Default for VrViewerConfig {
    fn default() -> Self {
        VrViewerConfig {
            movement_speed: 1.5,
            rotation_speed: Deg(30.0),
            enable_vsync: true,
            light_fields: Vec::new(),
        }
    }
}
