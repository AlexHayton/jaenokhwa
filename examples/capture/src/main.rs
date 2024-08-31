/*
 * Copyright 2024 Alex Hayton / The Jaenokhwa Contributors
 * Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

// Some assembly required. For developers 7 and up.

use clap::{Parser, Subcommand};
use color_eyre::Report;
use flume::Receiver;
use four_cc_nokhwa::FourCC;
use ggez::graphics::ImageFormat;
use ggez::{
    event::{run, EventHandler},
    graphics::{Canvas, Image},
    Context, ContextBuilder, GameError,
};
use jaenokhwa::query_devices;
use jaenokhwa::{
    buffer::FrameBuffer,
    pixel_format::{MJPEG, NV12, YUYV},
    utils::{CameraFormat, CameraIndex, RequestedFormat, RequestedFormatType, Resolution},
    CallbackCamera, Camera,
};
use std::fs::File;
use std::io::Write;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

struct CaptureState {
    receiver: Arc<Receiver<FrameBuffer>>,
    format: CameraFormat,
}

impl EventHandler<GameError> for CaptureState {
    fn update(&mut self, _ctx: &mut Context) -> Result<(), GameError> {
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> Result<(), GameError> {
        let buffer = self
            .receiver
            .recv()
            .map_err(|why| GameError::RenderError(why.to_string()))?;
        let image = Image::from_pixels(
            ctx,
            buffer.buffer(),
            ImageFormat::Rgba8Uint,
            self.format.width(),
            self.format.height(),
        );
        let canvas = Canvas::from_image(ctx, image, None);
        canvas.finish(ctx)
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone)]
enum IndexKind {
    String(String),
    Index(u32),
}

impl FromStr for IndexKind {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<u32>() {
            Ok(p) => Ok(IndexKind::Index(p)),
            Err(_) => Ok(IndexKind::String(s.to_string())),
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    ListDevices,
    ListProperties {
        device: Option<IndexKind>,
        kind: Option<PropertyKind>,
    },
    Stream {
        device: Option<IndexKind>,
        display: Option<bool>,
        requested: Option<RequestedCliFormat>,
    },
    Single {
        device: Option<IndexKind>,
        save: Option<String>,
        requested: Option<RequestedCliFormat>,
    },
}

enum CommandsProper {
    ListDevices,
    ListProperties {
        device: Option<IndexKind>,
        kind: PropertyKind,
    },
    Stream {
        device: Option<IndexKind>,
        display: bool,
        requested: Option<RequestedCliFormat>,
    },
    Single {
        device: Option<IndexKind>,
        requested: Option<RequestedCliFormat>,
        save: Option<String>,
    },
}

#[derive(Clone)]
struct RequestedCliFormat {
    format_type: String,
    format_option: Option<String>,
}

impl FromStr for RequestedCliFormat {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let splitted = s.split(':').collect::<Vec<&str>>();
        if splitted.is_empty() {
            return Err(Report::msg("empty string"));
        }

        Ok(RequestedCliFormat {
            format_type: splitted[0].to_string(),
            format_option: splitted.get(1).map(|x| x.to_string()),
        })
    }
}

impl RequestedCliFormat {
    pub fn make_requested(self) -> Option<RequestedFormat> {
        match self.format_type.as_str() {
            "AbsoluteHighestResolution" => Some(RequestedFormat::new(
                RequestedFormatType::AbsoluteHighestResolution,
            )),
            "AbsoluteHighestFrameRate" => Some(RequestedFormat::new(
                RequestedFormatType::AbsoluteHighestFrameRate,
            )),
            "HighestResolution" => {
                let fmtv = self.format_option.unwrap();
                let values = fmtv.split(',').collect::<Vec<&str>>();
                let x = values[0].parse::<u32>().unwrap();
                let y = values[1].parse::<u32>().unwrap();
                let resolution = Resolution::new(x, y);

                Some(RequestedFormat::new(
                    RequestedFormatType::HighestResolution(resolution),
                ))
            }
            "HighestFrameRate" => {
                let fps = self.format_option.unwrap().parse::<u32>().unwrap();

                Some(RequestedFormat::new(RequestedFormatType::HighestFrameRate(
                    fps,
                )))
            }
            "Closest" => {
                let fmtv = self.format_option.unwrap();
                let values = fmtv.split(',').collect::<Vec<&str>>();
                let x = values[0].parse::<u32>().unwrap();
                let y = values[1].parse::<u32>().unwrap();
                let fps = values[2].parse::<u32>().unwrap();
                let fourcc = values[3].parse::<FourCC>().unwrap();

                let resolution = Resolution::new(x, y);
                let camera_format = CameraFormat::new(resolution, fourcc, fps);
                Some(RequestedFormat::new(RequestedFormatType::Closest(
                    camera_format,
                )))
            }
            "None" => Some(RequestedFormat::new(RequestedFormatType::None)),
            _ => None,
        }
    }
}

#[derive(Copy, Clone)]
enum PropertyKind {
    All,
    Controls,
    CompatibleFormats,
}

impl FromStr for PropertyKind {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "All" | "ALL" | "all" => Ok(PropertyKind::All),
            "Controls" | "controls" | "CONTROLS" | "ctrls" => Ok(PropertyKind::Controls),
            "CompatibleFormats" | "compatibleformats" | "COMPATIBLEFORMATS" | "cf"
            | "compatfmts" => Ok(PropertyKind::CompatibleFormats),
            _ => Err(Report::msg(format!("unknown PropertyKind: {s}"))),
        }
    }
}

fn main() {
    nokhwa_main();
    std::thread::sleep(Duration::from_millis(2000));
}

fn nokhwa_main() {
    let cli = Cli::parse();

    let cmd = match &cli.command {
        Some(cmd) => cmd,
        None => {
            println!("Unknown command \"\". Do --help for info.");
            return;
        }
    };

    let cmd = match cmd {
        Commands::ListDevices => CommandsProper::ListDevices,
        Commands::ListProperties { device, kind } => CommandsProper::ListProperties {
            device: device.clone(),
            kind: match kind {
                Some(k) => *k,
                None => {
                    println!("Expected Positional Argument \"All\", \"Controls\", or \"CompatibleFormats\"");
                    return;
                }
            },
        },
        Commands::Stream {
            device,
            display,
            requested,
        } => CommandsProper::Stream {
            device: device.clone(),
            display: display.unwrap_or(false),
            requested: requested.clone(),
        },
        Commands::Single {
            device,
            save,
            requested,
        } => CommandsProper::Single {
            device: device.clone(),
            save: save.clone(),
            requested: requested.clone(),
        },
    };

    match cmd {
        CommandsProper::ListDevices => {
            let devices = query_devices().unwrap();
            println!("There are {} available cameras.", devices.len());
            for device in devices {
                println!("{device}");
            }
        }
        CommandsProper::ListProperties { device, kind } => {
            let index = match device.as_ref().unwrap_or(&IndexKind::Index(0)) {
                IndexKind::String(s) => CameraIndex::String(s.clone()),
                IndexKind::Index(i) => CameraIndex::Index(*i),
            };
            let mut camera =
                Camera::new(index, RequestedFormat::new(RequestedFormatType::None)).unwrap();
            match kind {
                PropertyKind::All => {
                    camera_print_controls(&camera);
                    camera_compatible_formats(&mut camera);
                }
                PropertyKind::Controls => {
                    camera_print_controls(&camera);
                }
                PropertyKind::CompatibleFormats => {
                    camera_compatible_formats(&mut camera);
                }
            }
        }
        CommandsProper::Stream {
            device,
            display,
            requested,
        } => {
            let requested = requested.as_ref().and_then(|x| x.clone().make_requested())
                .expect("Expected AbsoluteHighestResolution, AbsoluteHighestFrameRate, HighestResolution, HighestFrameRate, Exact, Closest, or None");

            let index = match device.as_ref().unwrap_or(&IndexKind::Index(0)) {
                IndexKind::String(s) => CameraIndex::String(s.clone()),
                IndexKind::Index(i) => CameraIndex::Index(*i),
            };

            if display {
                let (sender, receiver) = flume::unbounded();
                let (sender, receiver) = (Arc::new(sender), Arc::new(receiver));
                let sender_clone = sender.clone();

                let mut camera = CallbackCamera::new(index, requested, move |buf| {
                    sender_clone.send(buf).expect("Error sending frame!!!!");
                })
                .unwrap();

                let camera_info = camera.info().clone();
                let format = camera.camera_format().unwrap();

                camera.open_stream().unwrap();

                let cb = ContextBuilder::new(&camera_info.name(), "Nokhwa");
                let (ctx, el) = cb.build().unwrap();

                let state = CaptureState { receiver, format };

                run(ctx, el, state)
            } else {
                let mut cb = CallbackCamera::new(index, requested, |buf| {
                    println!("Captured frame of size {}", buf.buffer().len());
                })
                .unwrap();

                cb.open_stream().unwrap();
                #[allow(clippy::empty_loop)]
                loop {}
            }
        }
        CommandsProper::Single {
            device,
            save,
            requested,
        } => {
            let index = match device.as_ref().unwrap_or(&IndexKind::Index(0)) {
                IndexKind::String(s) => CameraIndex::String(s.clone()),
                IndexKind::Index(i) => CameraIndex::Index(*i),
            };

            #[allow(clippy::map_flatten)]
            let requested = requested.clone().map(|x| x.make_requested())
                .flatten()
                .expect("Expected AbsoluteHighestResolution, AbsoluteHighestFrameRate, HighestResolution, HighestFrameRate, Exact, Closest, or None");

            let mut camera = Camera::new(index, requested).unwrap();
            camera.open_stream().unwrap();
            let frame = camera.frame().unwrap();
            camera.stop_stream().unwrap();
            println!("Captured Single Frame of {}", frame.buffer().len());
            println!("Resolution: {}", frame.resolution());
            println!("Source Frame Format: {}", frame.source_frame_format());

            if let Some(path) = save {
                println!("Saving to {path}");
                let mut file = File::create(path).unwrap();
                let _ = file.write_all(frame.buffer());
            }
        }
    }
}

fn camera_print_controls(cam: &Camera) {
    let ctrls = cam.camera_controls().unwrap();
    let index = cam.index();
    println!("Controls for camera {index}");
    for ctrl in ctrls {
        println!("{ctrl}")
    }
}

fn camera_compatible_formats(cam: &mut Camera) {
    let frame_formats: Vec<FourCC> = vec![YUYV, NV12, MJPEG];
    for ffmt in frame_formats {
        if let Ok(compatible) = cam.compatible_list_by_resolution(ffmt) {
            println!("{ffmt}:");
            let mut formats = Vec::new();
            for (resolution, fps) in compatible {
                formats.push((resolution, fps));
            }
            formats.sort_by(|a, b| a.0.cmp(&b.0));
            for fmt in formats {
                let (resolution, res) = fmt;
                println!(" - {resolution}: {res:?}")
            }
        }
    }
}
