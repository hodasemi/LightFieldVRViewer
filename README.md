# LightFieldVRViewer

## Requirements

* Rust toolchain (https://www.rust-lang.org/) (tested with: rustc 1.41.0, stable branch)
* CMake (https://cmake.org/) (tested with: cmake 3.15.5)
* VK_NV_ray_tracing capable GPU ([Windows](https://vulkan.gpuinfo.org/listdevicescoverage.php?extension=VK_NV_ray_tracing&platform=windows), [Linux](https://vulkan.gpuinfo.org/listdevicescoverage.php?extension=VK_NV_ray_tracing&platform=linux))

## How to run

* Just run cargo: `cargo run --release`

## Controls for Desktop-Viewer

* `W`, `A`, `S`, `D` to navigate
* `Q`, `E` for rotation
* `Space`, `Ctrl` for up and down
* combine `Shift` with all, to move at a fourth of the speed
