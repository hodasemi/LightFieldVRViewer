use context::prelude::*;

use crate::error::{LightFieldError, Result};

use std::collections::HashMap;

const SCENE: &str = "scene";
const CATEGORY: &str = "category";
const DATE: &str = "date";
const VERSION: &str = "version";
const AUTHORS: &str = "authors";
const CONTACT: &str = "contact";
const CYCLES_SEED: &str = "cycles_seed";
const DISP_MIN: &str = "disp_min";
const DISP_MAX: &str = "disp_max";
const FRUSTUM_DISP_MIN: &str = "frustum_disp_min";
const FRUSTUM_DISP_MAX: &str = "frustum_disp_max";
const DEPTH_MAP_SCALE: &str = "depth_map_scale";

#[derive(Debug, PartialEq)]
pub struct Meta {
    pub scene_name: String,
    pub category: String,
    pub date: String,
    pub version: String,
    pub authors: Vec<String>,
    pub contact: String,
    pub cycles_seed: u64,
    pub disp_min: f32,
    pub disp_max: f32,
    pub frustum_disp_min: f32,
    pub frustum_disp_max: f32,
    pub depth_max_scale: f32,
}

impl Meta {
    pub fn load(data: &HashMap<String, Value>) -> Result<Self> {
        let author_string: String = data
            .get(AUTHORS)
            .ok_or(LightFieldError::config_loader("no authors present"))?
            .apply_value()?;

        let authors = author_string
            .split(",")
            .map(|author| author.trim().to_string())
            .collect();

        Ok(Meta {
            scene_name: data
                .get(SCENE)
                .ok_or(LightFieldError::config_loader("scene not present"))?
                .apply_value()?,
            category: data
                .get(CATEGORY)
                .ok_or(LightFieldError::config_loader("category not present"))?
                .apply_value()?,
            date: data
                .get(DATE)
                .ok_or(LightFieldError::config_loader("date not present"))?
                .apply_value()?,
            version: data
                .get(VERSION)
                .ok_or(LightFieldError::config_loader("version not present"))?
                .apply_value()?,
            authors,
            contact: data
                .get(CONTACT)
                .ok_or(LightFieldError::config_loader("contact not present"))?
                .apply_value()?,
            cycles_seed: data
                .get(CYCLES_SEED)
                .ok_or(LightFieldError::config_loader("cycles seed not present"))?
                .apply_value()?,
            disp_min: data
                .get(DISP_MIN)
                .ok_or(LightFieldError::config_loader("disp min not present"))?
                .apply_value()?,
            disp_max: data
                .get(DISP_MAX)
                .ok_or(LightFieldError::config_loader("disp max not present"))?
                .apply_value()?,
            frustum_disp_min: data
                .get(FRUSTUM_DISP_MIN)
                .ok_or(LightFieldError::config_loader(
                    "frustum disp min not present",
                ))?
                .apply_value()?,
            frustum_disp_max: data
                .get(FRUSTUM_DISP_MAX)
                .ok_or(LightFieldError::config_loader(
                    "frustum disp max not present",
                ))?
                .apply_value()?,
            depth_max_scale: data
                .get(DEPTH_MAP_SCALE)
                .ok_or(LightFieldError::config_loader(
                    "depth max scale not present",
                ))?
                .apply_value()?,
        })
    }
}
