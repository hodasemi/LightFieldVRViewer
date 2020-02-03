use cgmath::Deg;
use context::prelude::*;

pub mod config;
pub mod feet_renderer;
pub mod interpolation;
pub mod light_field;
pub mod light_field_viewer;
pub mod rasterizer;
pub mod view_emulator;

use light_field::LightField;
use light_field_viewer::LightFieldViewer;

use std::sync::Arc;
use std::thread;

fn main() -> VerboseResult<()> {
    let viewer_config = VrViewerConfig::load("settings.conf")?;

    let context = create_context(viewer_config.force_desktop, viewer_config.enable_vsync)?;

    // spawn threads to load light fields
    let join_handles: Vec<thread::JoinHandle<VerboseResult<LightField>>> = viewer_config
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

    for join_handle in join_handles.into_iter() {
        match join_handle.join()? {
            Ok(light_field) => light_fields.push(light_field),
            Err(msg) => println!("{}", msg),
        }
    }

    if light_fields.is_empty() {
        println!("no fields loaded!\nclosing ...");
        create_error!("");
    }

    // create viewer
    let light_field_viewer = LightFieldViewer::new(
        &context,
        light_fields,
        viewer_config.rotation_speed,
        viewer_config.movement_speed,
        viewer_config.enable_feet,
        viewer_config.enable_frustum,
    )?;

    println!("created viewer!");

    // add viewer to context
    context.set_context_object(Some(light_field_viewer.clone()))?;
    context.render_core().add_scene(light_field_viewer)?;

    // loop
    context.run()?;

    Ok(())
}

fn create_context(force_desktop: bool, enable_vsync: bool) -> VerboseResult<Arc<Context>> {
    if force_desktop {
        create_desktop_context(enable_vsync)
    } else {
        match create_vr_context() {
            Ok(context) => Ok(context),
            Err(msg) => {
                println!("{:?}", msg);
                println!("failed creating VR Context");

                create_desktop_context(enable_vsync)
            }
        }
    }
}

fn create_vr_context() -> VerboseResult<Arc<Context>> {
    Context::new()
        .set_vulkan_debug_info(VulkanDebugInfo {
            debugging: true,
            renderdoc: false,
            steam_layer: false,
            use_util: false,
            verbose: false,
        })
        .set_vr_mode(VRMode::OpenVR)
        .set_render_core_info(
            VK_FORMAT_R8G8B8A8_UNORM,
            VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_STORAGE_BIT,
            true,
        )
        .build()
}

fn create_desktop_context(enable_vsync: bool) -> VerboseResult<Arc<Context>> {
    let context_builder = Context::new()
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
        .set_render_core_info(
            VK_FORMAT_R8G8B8A8_SRGB,
            VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_STORAGE_BIT,
            enable_vsync,
        )
        .enable_keyboard();

    context_builder.build()
}

const DESKTOP_META: &str = "Desktop";
const INFO_META: &str = "Info";

const MOVEMENT_SPEED: &str = "movement_speed";
const ROTATION_SPEED: &str = "rotation_speed";
const VSYNC: &str = "enable_vsync";
const LIGHT_FIELDS: &str = "light_fields";
const ENABLE_FEET: &str = "enable_feet";
const ENABLE_FRUSTUM: &str = "enable_frustum";
const FORCE_DESKTOP: &str = "force";

struct VrViewerConfig {
    // in meter per second
    movement_speed: f32,

    // in degrees per second
    rotation_speed: Deg<f32>,

    // only in desktop mode
    enable_vsync: bool,

    light_fields: Vec<String>,
    enable_feet: bool,
    enable_frustum: bool,
    force_desktop: bool,
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

            if let Some(value) = info.get(FORCE_DESKTOP) {
                config.force_desktop = value.to_value()?;
            }
        }

        if let Some(info) = config_data.get(INFO_META) {
            if let Some(value) = info.get(LIGHT_FIELDS) {
                config.light_fields = value.to_array()?;
            }

            if let Some(value) = info.get(ENABLE_FEET) {
                config.enable_feet = value.to_value()?;
            }

            if let Some(value) = info.get(ENABLE_FRUSTUM) {
                config.enable_frustum = value.to_value()?;
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
            enable_frustum: true,
            enable_feet: true,
            force_desktop: false,
        }
    }
}
