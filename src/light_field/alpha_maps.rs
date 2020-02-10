use context::prelude::*;

use std::fs::File;
use std::io::BufReader;
use std::slice::Iter;

use pxm::PFM;

#[derive(Debug, Clone)]
pub struct AlphaMap {
    data: Vec<Vec<bool>>,

    depth: Vec<f32>,
}

impl AlphaMap {
    fn new(width: usize, height: usize) -> Self {
        AlphaMap {
            data: vec![vec![false; height as usize]; width as usize],

            depth: Vec::new(),
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

    pub fn depth_values(&self) -> &Vec<f32> {
        &self.depth
    }
}

pub struct AlphaMaps {
    maps: Vec<AlphaMap>,
}

impl AlphaMaps {
    pub fn new(
        depth_pfm: PFM,
        layer_count: usize,
        minimal_depth: f32,
        threshold: f32,
    ) -> VerboseResult<Self> {
        let mut alpha_maps = vec![AlphaMap::new(depth_pfm.width, depth_pfm.height); layer_count];

        for (index, depth) in depth_pfm.data.iter().enumerate() {
            let (x, y) = Self::to_xy(depth_pfm.height, index);

            for (layer_index, alpha_map) in alpha_maps.iter_mut().enumerate() {
                if *depth >= (minimal_depth + layer_index as f32 * threshold)
                    && *depth < (minimal_depth + (layer_index as f32 + 1.0) * threshold)
                {
                    alpha_map.data[x][y] = true;
                    alpha_map.depth.push(*depth);
                }
            }
        }

        Ok(AlphaMaps { maps: alpha_maps })
    }

    // pub fn load_depth(mut self, path: &str) -> VerboseResult<Self> {
    //     let depth_pfm = Self::open_pfm_file(&path)?;

    //     let mut depths = Vec::new();

    //     for depth in depth_pfm.data.iter() {
    //         if !depths.contains(depth) {
    //             depths.push(*depth);
    //         }
    //     }

    //     println!("depths: {:#?}", depths);

    //     for alpha_map in self.maps.iter_mut() {
    //         let mut depth_values = BinaryHeap::with_capacity(alpha_map.data.len());

    //         alpha_map.for_each_alpha(|x, y| {
    //             let index = Self::to_index(&depth_pfm, x, y);

    //             depth_values.push(OrderedFloat(depth_pfm.data[index]));
    //         });

    //         if !depth_values.is_empty() {
    //             let depth_vec = depth_values
    //                 .into_sorted_vec()
    //                 .iter()
    //                 .map(|ordered_float| ordered_float.into_inner())
    //                 .collect();

    //             alpha_map.depth = Some(depth_vec);
    //         }
    //     }

    //     Ok(self)
    // }

    pub fn iter(&self) -> Iter<'_, AlphaMap> {
        self.maps.iter()
    }

    // pub fn len(&self) -> usize {
    //     self.maps.len()
    // }

    #[inline]
    pub fn open_pfm_file(path: &str) -> VerboseResult<PFM> {
        let pfm_file = File::open(path)?;
        let mut pfm_bufreader = BufReader::new(pfm_file);

        Ok(PFM::read_from(&mut pfm_bufreader)?)
    }

    #[inline]
    pub fn to_xy(height: usize, index: usize) -> (usize, usize) {
        let y = (index as f32 / height as f32).floor() as usize;
        let x = index - (y * height);

        (x, y)
    }

    // #[inline]
    // fn to_index(pfm: &PFM, x: usize, y: usize) -> usize {
    //     y * pfm.height + x
    // }
}
