use std::{
    ops::Deref,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use eframe::egui::{self, ColorImage};
use ffmpeg_next::format::Pixel;
use nokhwa::{
    convert_to_rgb::ConvertToRgb,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    CallbackCamera,
};
use once_cell::sync::Lazy;

static mut TEXTURE: Option<Arc<Mutex<egui::TextureHandle>>> = None;
static mut FRAME_COUNTER: u64 = 0;
static mut LAST_STATS: once_cell::sync::Lazy<Instant> = Lazy::new(|| Instant::now());
static STATS_INTERVAL_SECS: u64 = 10;
static mut IMAGE_CONVERT_TIMES: Vec<Duration> = Vec::new();
static mut EGUI_TEXTURE_TIMES: Vec<Duration> = Vec::new();

#[derive(Default)]
struct MyApp {
    camera: Option<CallbackCamera>,
}

impl MyApp {
    fn new(ctx: &eframe::CreationContext<'_>) -> Self {
        let index = CameraIndex::Index(0);
        // request the absolute highest frame rate camera (stress test)
        let requested = RequestedFormat::new(RequestedFormatType::AbsoluteHighestResolution);
        let mut camera = CallbackCamera::new(index, requested, move |framebuffer| {
            let width = framebuffer.width();
            let height = framebuffer.height();
            if unsafe { FRAME_COUNTER } == 0 {
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
                println!("Will output stats every 10 seconds");
            }

            let start_image_convert = Instant::now();
            let rgb_data = framebuffer.convert_to_rgb(Pixel::RGB24);
            unsafe { IMAGE_CONVERT_TIMES.push(start_image_convert.elapsed()) };

            let size = [width as usize, height as usize];
            let start_egui_texture = Instant::now();
            
            let color_image = ColorImage::from_rgb(size, &rgb_data);

            unsafe { EGUI_TEXTURE_TIMES.push(start_egui_texture.elapsed()) };

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

            unsafe {
                if LAST_STATS.elapsed() >= Duration::from_secs(STATS_INTERVAL_SECS) {
                    let total_image_convert_time: Duration = IMAGE_CONVERT_TIMES.iter().sum();
                    let average_image_convert_time =
                        total_image_convert_time / IMAGE_CONVERT_TIMES.len() as u32;
                    println!(
                        "Average YUV => RGB conversion time: {:?}",
                        average_image_convert_time
                    );
                    IMAGE_CONVERT_TIMES.clear();

                    let total_egui_texture_time: Duration = EGUI_TEXTURE_TIMES.iter().sum();
                    let average_egui_texture_time =
                        total_egui_texture_time / EGUI_TEXTURE_TIMES.len() as u32;
                    println!(
                        "Average egui texture creation time: {:?}",
                        average_egui_texture_time
                    );
                    EGUI_TEXTURE_TIMES.clear();

                    LAST_STATS = Lazy::new(|| Instant::now());
                    println!("Average frame rate: {:?}", FRAME_COUNTER as f32 / 10.0);
                }
            }
        })
        .unwrap();

        let compatible_fourcc = camera.compatible_fourcc().unwrap();
        println!("Compatible FourCCs: {:?}", compatible_fourcc);

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
        ctx.request_repaint();
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
