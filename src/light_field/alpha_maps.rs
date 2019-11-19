use context::prelude::*;

use std::fs::File;
use std::io::BufReader;
use std::slice::Iter;

use pxm::PFM;

// 1 milli meter
const EQ_THRESHOLD: f32 = 0.001;

#[derive(Clone, Debug)]
pub struct AlphaMap {
    data: Vec<Vec<bool>>,

    depth: Option<f32>,
}

impl AlphaMap {
    fn new(width: usize, height: usize) -> Self {
        AlphaMap {
            data: vec![vec![false; height as usize]; width as usize],

            depth: None,
        }
    }

    pub fn for_each_alpha<F>(&self, mut f: F)
    where
        F: FnMut(usize, usize) -> (),
    {
        for (x, row) in self.data.iter().enumerate() {
            for (y, alpha_value) in row.iter().enumerate() {
                if *alpha_value {
                    f(x, y);
                }
            }
        }
    }

    pub fn depth(&self) -> Option<f32> {
        self.depth
    }
}

pub struct AlphaMaps {
    maps: Vec<AlphaMap>,
}

impl AlphaMaps {
    pub fn new(path: &str, alpha_map_count: usize, epsilon: f32) -> VerboseResult<Self> {
        let pfm = Self::open_pfm_file(path)?;

        let mut alpha_maps = vec![AlphaMap::new(pfm.width, pfm.height); alpha_map_count];

        for (index, disp_data) in pfm.data.iter().enumerate() {
            let (x, y) = Self::to_xy(&pfm, index);

            for (disparity, alpha_map) in alpha_maps.iter_mut().enumerate() {
                if (disp_data.abs() - disparity as f32).abs() <= epsilon {
                    alpha_map.data[x][y] = true;
                }
            }
        }

        Ok(AlphaMaps { maps: alpha_maps })
    }

    pub fn load_depth(mut self, path: &str) -> VerboseResult<Self> {
        let depth_pfm = Self::open_pfm_file(&path)?;

        for alpha_map in self.maps.iter_mut() {
            let mut depth_values = Vec::new();

            alpha_map.for_each_alpha(|x, y| {
                let index = Self::to_index(&depth_pfm, x, y);

                let depth = depth_pfm.data[index];

                if !depth_values.contains(&depth) {
                    depth_values.push(depth);
                }
            });

            if !depth_values.is_empty() {
                let mut base_depth = depth_values[0];

                for depth in depth_values.iter() {
                    if !Self::check_eq(base_depth, *depth) {
                        create_error!("depth value are too far apart");
                    }

                    base_depth = (base_depth + depth) / 2.0;
                }

                // max out the depth at 500 meters
                base_depth = base_depth.min(500.0);

                alpha_map.depth = Some(base_depth);
            }
        }

        Ok(self)
    }

    pub fn iter(&self) -> Iter<'_, AlphaMap> {
        self.maps.iter()
    }

    pub fn len(&self) -> usize {
        self.maps.len()
    }

    #[inline]
    fn open_pfm_file(path: &str) -> VerboseResult<PFM> {
        let pfm_file = File::open(path)?;
        let mut pfm_bufreader = BufReader::new(pfm_file);

        Ok(PFM::read_from(&mut pfm_bufreader)?)
    }

    #[inline]
    fn to_xy(pfm: &PFM, index: usize) -> (usize, usize) {
        let y = (index as f32 / pfm.height as f32).floor() as usize;
        let x = index - (y * pfm.height);

        (x, y)
    }

    #[inline]
    fn to_index(pfm: &PFM, x: usize, y: usize) -> usize {
        y * pfm.height + x
    }

    #[inline]
    fn check_eq(f1: f32, f2: f32) -> bool {
        (f1 - f2) < EQ_THRESHOLD && (f2 - f1) < EQ_THRESHOLD
    }
}
