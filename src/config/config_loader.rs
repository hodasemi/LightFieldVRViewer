use context::prelude::*;

use super::{extrinsic::Extrinsic, intrinsic::Intrinsic, meta::Meta};

const META_TAG: &str = "meta";
const EXTRINSIC: &str = "extrinsic";
const INTRINSIC: &str = "intrinsic";

#[derive(Debug, PartialEq)]
pub struct Config {
    pub meta: Meta,
    pub intrinsic: Intrinsic,
    pub extrinsic: Extrinsic,
}

impl Config {
    pub fn load(path: &str) -> VerboseResult<Config> {
        let config = ConfigHandler::read_config(path)?;

        println!("{:#?}", config);

        let intrinsic = Intrinsic::load(
            config
                .get(INTRINSIC)
                .ok_or("intrinsic tag is missing in config")?,
        )?;

        let extrinsic = Extrinsic::load(
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
            intrinsic,
            extrinsic,
        })
    }
}
