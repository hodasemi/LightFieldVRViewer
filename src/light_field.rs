use context::prelude::*;

use super::config::Config;

use std::path::Path;
use std::sync::Arc;
use std::thread;

pub struct LightField {
    pub config: Config,

    pub input_images: Vec<Vec<Option<Arc<Image>>>>,
}

impl LightField {
    pub fn new(context: &Arc<Context>, dir: &str) -> VerboseResult<Self> {
        let config = Config::load(&format!("{}/parameters.cfg", dir))?;

        let mut input_images = vec![
            vec![None; config.extrinsics.horizontal_camera_count as usize];
            config.extrinsics.vertical_camera_count as usize
        ];

        let mut threads = Vec::with_capacity(
            (config.extrinsics.horizontal_camera_count * config.extrinsics.vertical_camera_count)
                as usize,
        );

        let mut total_index = 0;

        for (y, col) in input_images.iter().enumerate() {
            for (x, _image) in col.iter().enumerate() {
                let queue = context.queue().clone();
                let device = context.device().clone();

                let meta_image_width = config.intrinsics.image_width;
                let meta_image_height = config.intrinsics.image_height;

                let dir = dir.to_string();

                threads.push(thread::spawn(move || {
                    let path = format!("{}/input_Cam{:03}.png", dir, total_index);

                    let image = if Path::new(&path).exists() {
                        println!("loading image {}", path);

                        let image = Image::from_file(&path)?
                            .nearest_sampler()
                            .build(&device, &queue)?;

                        println!("loading finished ({})", path);

                        // check if texture dimensions match meta information
                        if image.width() != meta_image_width || image.height() != meta_image_height
                        {
                            create_error!(format!("Image ({}) has a not expected extent", path));
                        }

                        image
                    } else {
                        create_error!(format!("{} does not exist", path));
                    };

                    Ok((image, x, y))
                }));

                total_index += 1;
            }
        }

        for thread in threads {
            if let Ok(thread_result) = thread.join() {
                if let Ok((image, x, y)) = thread_result {
                    input_images[x][y] = Some(image);
                }
            }
        }

        println!("finished loading light field {}", dir);

        Ok(LightField {
            config,
            input_images,
        })
    }
}
