use cgmath::Deg;
use context::prelude::*;

/// Rust equivalent of the parameters of a light field
pub mod config;

/// Debug utilities, used in development phase
pub mod debug;

/// Feet and outline renderer
pub mod feet_renderer;

/// CPU side of per frame calculations
pub mod interpolation;

/// Intermediate struct of a light field
pub mod light_field;

/// Viewer, main struct that keeps everything together
pub mod light_field_viewer;

/// Rasterizer Vulkan handles for feet and outline rendering
pub mod rasterizer;

/// User emulator for desktop viewer
pub mod view_emulator;

use light_field::LightField;
use light_field_viewer::LightFieldViewer;

use std::sync::Arc;
use std::thread;

/// `main` entry point of the program
fn main() -> VerboseResult<()> {
    let viewer_config = VrViewerConfig::load("settings.conf")?;

    let context = create_context(
        viewer_config.force_desktop,
        viewer_config.enable_vsync,
        viewer_config.window_width,
        viewer_config.window_height,
    )?;

    let number_of_slices = viewer_config.number_of_slices;

    // spawn threads to load light fields
    let join_handles: Vec<thread::JoinHandle<VerboseResult<LightField>>> = viewer_config
        .light_fields
        .iter()
        .cloned()
        .map(|field_path| {
            let context_clone = context.clone();

            thread::spawn(move || LightField::new(&context_clone, &field_path, number_of_slices))
        })
        .collect();

    // wait for thread to join
    let mut light_fields = Vec::new();

    for join_handle in join_handles.into_iter() {
        match join_handle.join()? {
            Ok(light_field) => {
                if !light_field.is_empty() {
                    light_fields.push(light_field);
                }
            }
            Err(msg) => println!("{}", msg),
        }
    }

    if light_fields.is_empty() {
        println!("no fields loaded!\nclosing ...");
        create_error!("");
    }

    // create viewer
    let light_field_viewer = match LightFieldViewer::new(
        &context,
        light_fields,
        viewer_config.rotation_speed,
        viewer_config.movement_speed,
        viewer_config.enable_feet,
        viewer_config.enable_frustum,
        number_of_slices,
    ) {
        Ok(viewer) => viewer,
        Err(err) => {
            println!("{}", err.message());
            return Ok(());
        }
    };

    println!("created viewer!");

    // add viewer to context
    context.set_context_object(Some(light_field_viewer.clone()))?;
    context.render_core().add_scene(light_field_viewer)?;

    // loop
    context.run()?;

    Ok(())
}

/// Creates the context handle, based on input parameters
///
/// # Arguments
///
/// * `force_desktop` enables desktop backend
/// * `enable_vsync` enables vsync
/// * `width` width of the window if desktop backend is enabled
/// * `height` height of the window if desktop backend is enabled
fn create_context(
    force_desktop: bool,
    enable_vsync: bool,
    width: u32,
    height: u32,
) -> VerboseResult<Arc<Context>> {
    if force_desktop {
        create_desktop_context(enable_vsync, width, height)
    } else {
        match create_vr_context() {
            Ok(context) => Ok(context),
            Err(msg) => {
                println!("{:?}", msg);
                println!("failed creating VR Context");

                create_desktop_context(enable_vsync, width, height)
            }
        }
    }
}

/// Creates context handle with OpenVR backend
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
        .set_render_core_info(
            VK_FORMAT_R8G8B8A8_UNORM,
            VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_STORAGE_BIT,
            true,
        )
        .build()
}

/// Creates context handle with desktop window backend
fn create_desktop_context(
    enable_vsync: bool,
    width: u32,
    height: u32,
) -> VerboseResult<Arc<Context>> {
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
            width,
            height,
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

#[doc(hidden)]
const DESKTOP_META: &str = "Desktop";

#[doc(hidden)]
const INFO_META: &str = "Info";

#[doc(hidden)]
const MOVEMENT_SPEED: &str = "movement_speed";

#[doc(hidden)]
const ROTATION_SPEED: &str = "rotation_speed";

#[doc(hidden)]
const VSYNC: &str = "enable_vsync";

#[doc(hidden)]
const LIGHT_FIELDS: &str = "light_fields";

#[doc(hidden)]
const ENABLE_FEET: &str = "enable_feet";

#[doc(hidden)]
const ENABLE_FRUSTUM: &str = "enable_frustum";

#[doc(hidden)]
const FORCE_DESKTOP: &str = "force";

#[doc(hidden)]
const NUMBER_OF_SLICES: &str = "slice_count";

#[doc(hidden)]
const WINDOW_WIDTH: &str = "width";

#[doc(hidden)]
const WINDOW_HEIGHT: &str = "height";

/// Config struct, equivalent to the parameters file
struct VrViewerConfig {
    // in meter per second
    movement_speed: f32,

    // in degrees per second
    rotation_speed: Deg<f32>,

    // only in desktop mode
    enable_vsync: bool,

    window_width: u32,
    window_height: u32,

    light_fields: Vec<String>,
    number_of_slices: usize,
    enable_feet: bool,
    enable_frustum: bool,
    force_desktop: bool,
}

impl VrViewerConfig {
    /// loads config file parameters into a `VrViewerConfig` struct
    ///
    /// # Parameters
    ///
    /// * `file` Path to the configuration file
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

            if let Some(value) = info.get(WINDOW_WIDTH) {
                config.window_width = value.to_value()?;
            }

            if let Some(value) = info.get(WINDOW_HEIGHT) {
                config.window_height = value.to_value()?;
            }
        }

        if let Some(info) = config_data.get(INFO_META) {
            if let Some(value) = info.get(NUMBER_OF_SLICES) {
                config.number_of_slices = value.to_value()?;
            }

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
            number_of_slices: 5,
            enable_frustum: true,
            enable_feet: true,
            force_desktop: false,
            window_width: 1280,
            window_height: 720,
        }
    }
}
