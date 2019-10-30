use context::prelude::*;

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
    pub fn load(data: &HashMap<String, Value>) -> VerboseResult<Self> {
        let author_string: String = data.get(AUTHORS).ok_or("no authors present")?.to_value()?;

        let authors = author_string
            .split(",")
            .map(|author| author.trim().to_string())
            .collect();

        Ok(Meta {
            scene_name: data.get(SCENE).ok_or("scene not present")?.to_value()?,
            category: data
                .get(CATEGORY)
                .ok_or("category not present")?
                .to_value()?,
            date: data.get(DATE).ok_or("date not present")?.to_value()?,
            version: data.get(VERSION).ok_or("version not present")?.to_value()?,
            authors,
            contact: data.get(CONTACT).ok_or("contact not present")?.to_value()?,
            cycles_seed: data
                .get(CYCLES_SEED)
                .ok_or("cycles seed not present")?
                .to_value()?,
            disp_min: data
                .get(DISP_MIN)
                .ok_or("disp min not present")?
                .to_value()?,
            disp_max: data
                .get(DISP_MAX)
                .ok_or("disp max not present")?
                .to_value()?,
            frustum_disp_min: data
                .get(FRUSTUM_DISP_MIN)
                .ok_or("frustum disp min not present")?
                .to_value()?,
            frustum_disp_max: data
                .get(FRUSTUM_DISP_MAX)
                .ok_or("frustum disp max not present")?
                .to_value()?,
            depth_max_scale: data
                .get(DEPTH_MAP_SCALE)
                .ok_or("depth max scale not present")?
                .to_value()?,
        })
    }
}
