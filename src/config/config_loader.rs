use context::prelude::*;

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
}
