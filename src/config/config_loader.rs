use context::prelude::*;

use cgmath::{Array, Vector3};

use super::{extrinsic::Extrinsic, intrinsic::Intrinsic, meta::Meta};

const META_TAG: &str = "meta";
const EXTRINSIC: &str = "extrinsics";
const INTRINSIC: &str = "intrinsics";

#[derive(Debug, PartialEq)]
pub struct Config {
    pub meta: Meta,
    pub intrinsics: Intrinsic,
    pub extrinsics: Extrinsic,
}

impl Config {
    pub fn load(path: &str) -> VerboseResult<Config> {
        let config = ConfigHandler::read_config(path)?;

        let intrinsics = Intrinsic::load(
            config
                .get(INTRINSIC)
                .ok_or("intrinsic tag is missing in config")?,
        )?;

        let extrinsics = Extrinsic::load(
            config
                .get(EXTRINSIC)
                .ok_or("extrinsic tag is missing in config")?,
        )?;

        let meta = Meta::load(
            config
                .get(META_TAG)
                .ok_or("meta tag is missing in config")?,
        )?;

        Ok(Config {
            meta,
            intrinsics,
            extrinsics,
        })
    }

    #[inline]
    pub fn swap_axis(mut v: Vector3<f32>) -> Vector3<f32> {
        v.swap_elements(1, 2);
        v.z = -v.z;

        v
    }
}
