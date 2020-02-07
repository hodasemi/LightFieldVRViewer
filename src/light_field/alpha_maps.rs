use context::prelude::*;

use std::collections::BinaryHeap;
use std::fs::File;
use std::io::BufReader;
use std::slice::Iter;

use ordered_float::OrderedFloat;
use pxm::PFM;
use std::cmp::max;

#[derive(Debug, Clone)]
pub struct AlphaMap {
    data: Vec<Vec<bool>>,

    depth: Option<Vec<f32>>,
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

    pub fn depth_values(&self) -> &Option<Vec<f32>> {
        &self.depth
    }
}

pub struct AlphaMaps {
    maps: Vec<AlphaMap>,
}

impl AlphaMaps {
    pub fn new(path: &str, alpha_map_count: usize, epsilon: f32) -> VerboseResult<Self> {
        let pfm = Self::open_pfm_file(path)?;

        let mut alpha_maps = vec![AlphaMap::new(pfm.width, pfm.height); max(alpha_map_count, 1)];

        for (index, disp_data) in pfm.data.iter().enumerate() {
            let (x, y) = Self::to_xy(&pfm, index);

            for (disparity_layer_index, alpha_map) in alpha_maps.iter_mut().enumerate() {
                if (disp_data - disparity_layer_index as f32).abs() <= epsilon {
                    alpha_map.data[x][y] = true;
                }
            }
        }

        Ok(AlphaMaps { maps: alpha_maps })
    }

    pub fn load_depth(mut self, path: &str) -> VerboseResult<Self> {
        let depth_pfm = Self::open_pfm_file(&path)?;

        for alpha_map in self.maps.iter_mut() {
            let mut depth_values = BinaryHeap::with_capacity(alpha_map.data.len());

            alpha_map.for_each_alpha(|x, y| {
                let index = Self::to_index(&depth_pfm, x, y);

                depth_values.push(OrderedFloat(depth_pfm.data[index]));
            });

            if !depth_values.is_empty() {
                let depth_vec = depth_values
                    .into_sorted_vec()
                    .iter()
                    .map(|ordered_float| ordered_float.into_inner())
                    .collect();

                alpha_map.depth = Some(depth_vec);
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
}
