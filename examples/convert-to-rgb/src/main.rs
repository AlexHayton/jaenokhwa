use std::{ops::Deref, sync::{Arc, Mutex}, time::Instant};

use eframe::egui::{self, ColorImage};
use image::{ImageBuffer, RgbImage};
use nokhwa::{
    convert_to_rgb::ConvertToRgb,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    CallbackCamera
};

static mut TEXTURE: Option<Arc<Mutex<egui::TextureHandle>>> = None;
static mut FRAME_COUNTER: u32 = 0;

#[derive(Default)]
struct MyApp {
    camera: Option<CallbackCamera>,
}

impl MyApp {
    fn new(ctx: &eframe::CreationContext<'_>) -> Self {
        let index = CameraIndex::Index(0);
        // request the absolute highest frame rate camera (stress test)
        let requested = RequestedFormat::new(RequestedFormatType::AbsoluteHighestFrameRate);
        let mut camera = CallbackCamera::new(index, requested, move |framebuffer| {
            let width = framebuffer.width();
            let height = framebuffer.height();
            println!(
                "Original Framebuffer format: {:?} {}x{}",
                framebuffer.source_frame_format(),
                framebuffer.width(),
                framebuffer.height()
            );
            println!(
                "Original Framebuffer size in bytes: {}",
                framebuffer.buffer().len()
            );

            let start = Instant::now();
            let rgb_data = framebuffer.convert_to_rgb_bytes();
            println!("Conversion took: {:?}", start.elapsed());
            println!("Converted RGB buffer size in bytes: {}", rgb_data.len());
            let imagebuffer: RgbImage =
                ImageBuffer::from_raw(width, height, rgb_data).expect("Failed to load image");

            // Save as JPEG
            let yuv_image = turbojpeg::YuvImage {
                pixels: framebuffer.buffer(),
                width: width as usize,
                height: height as usize,
                align: 2,
                subsamp: turbojpeg::Subsamp::Sub2x2,
            };
            let jpeg_data = turbojpeg::compress_yuv(yuv_image.as_deref(), 90).unwrap();

            // write the JPEG to disk
            let result = std::fs::write(std::env::temp_dir().join("same_parrots.jpg"), &jpeg_data);
            if let Err(e) = result {
                println!("Failed to write JPEG: {}", e);
            }

            let image = image::DynamicImage::ImageRgb8(imagebuffer);
            let rgba_image = image.to_rgba8();
            let size = [width as usize, height as usize];
            let pixels = rgba_image.into_vec();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
            match unsafe { TEXTURE.as_ref() } {
                None => {
                    panic!("This should never happen");
                }
                Some(texture) => {
                    let options = egui::TextureOptions::default();
                    let mut mutable = texture.lock().unwrap();
                    mutable.set(color_image, options);
                    unsafe { FRAME_COUNTER += 1 };
                }
            }
        })
        .unwrap();

        let camera_format = camera.camera_format();
        let frame_format = camera.frame_format();
        let resolution = camera.resolution().unwrap();
        println!("Camera format: {:?}", camera_format);
        println!("Frame format: {:?}", frame_format);
        println!("Camera resolution: {:?}", resolution);

        // Allocate a texture:
        let name = "Camera Image";
        let options = egui::TextureOptions::default();
        let blankimage = ColorImage::new(
            [resolution.width() as usize, resolution.height() as usize],
            egui::Color32::BLACK,
        );
        unsafe {
            TEXTURE = Some(Arc::new(Mutex::new(
                ctx.egui_ctx.load_texture(name, blankimage, options),
            )));
        };

        camera.open_stream().unwrap();
        println!("Camera opened successfully");

        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        let mut app = Self::default();
        app.camera = Some(camera);
        app
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| match unsafe { TEXTURE.as_ref() } {
            None => {
                ui.label("Loading...");
            }
            Some(texture) => {
                ui.heading("Nice camera");
                let locked_texture = texture.lock().ok().unwrap();
                ui.add(
                    egui::Image::new(locked_texture.deref())
                        .max_width(800.0)
                        .rounding(10.0),
                );
            }
        });
    }
}

fn main() {
    let options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "Camera Feed",
        options,
        Box::new(|cc| Ok(Box::new(MyApp::new(cc)))),
    );
}
