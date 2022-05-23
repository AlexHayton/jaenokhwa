/*
 * Copyright 2021 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
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

use clap::{App, Arg};
use glium::{
    implement_vertex, index::PrimitiveType, program, texture::RawImage2d, uniform, Display,
    IndexBuffer, Surface, Texture2d, VertexBuffer,
};
use glutin::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    ContextBuilder,
};
use nokhwa::{nokhwa_initialize, query_devices, Camera, CaptureAPIBackend, FrameFormat};
use std::time::Instant;

#[derive(Copy, Clone)]
pub struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

fn main() {
    let matches = App::new("nokhwa-example")
        .version("0.1.0")
        .author("l1npengtul <l1npengtul@protonmail.com> and the Nokhwa Contributers")
        .about("Example program using Nokhwa")
        .arg(Arg::with_name("query")
            .short("q")
            .long("query")
            .value_name("BACKEND")
            // TODO: Update as new backends are added!
            .help("Query the system? Pass AUTO for automatic backend, UVC to query using UVC, V4L to query using Video4Linux, GST to query using Gstreamer, MSMF to query using Media Foundation.. Will post the list of availible devices.")
            .default_value("AUTO")
            .takes_value(true))
        .arg(Arg::with_name("capture")
            .short("c")
            .long("capture")
            .value_name("LOCATION")
            .help("Capture from device? Pass the device index or string. Defaults to 0. If the input is not a number, it will be assumed an IPCamera")
            .default_value("0")
            .takes_value(true))
        .arg(Arg::with_name("query-device")
            .short("s")
            .long("query-device")
            .help("Show device queries from `compatible_fourcc` and `compatible_list_by_resolution`. Requires -c to be passed to work.")
            .takes_value(false))
        .arg(Arg::with_name("width")
            .short("w")
            .long("width")
            .value_name("WIDTH")
            .help("Set width of capture. Does nothing if -c flag is not set.")
            .default_value("640")
            .takes_value(true))
        .arg(Arg::with_name("height")
            .short("h")
            .long("height")
            .value_name("HEIGHT")
            .help("Set height of capture. Does nothing if -c flag is not set.")
            .default_value("480")
            .takes_value(true))
        .arg(Arg::with_name("framerate")
            .short("rate")
            .long("framerate")
            .value_name("FRAMES_PER_SECOND")
            .help("Set FPS of capture. Does nothing if -c flag is not set.")
            .default_value("15")
            .takes_value(true))
        .arg(Arg::with_name("format")
            .short("4cc")
            .long("format")
            .value_name("FORMAT")
            .help("Set format of capture. Does nothing if -c flag is not set. Possible values are MJPG and YUYV. Will be ignored if not either. Ignored by GStreamer backend.")
            .default_value("MJPG")
            .takes_value(true))
        .arg(Arg::with_name("capture-backend")
            .short("b")
            .long("backend")
            .value_name("BACKEND")
            .help("Set the capture backend. Pass AUTO for automatic backend, UVC to query using UVC, V4L to query using Video4Linux, GST to query using Gstreamer, OPENCV to use OpenCV,  MSMF to use Media Foundation")
            .default_value("AUTO")
            .takes_value(true))
        .arg(Arg::with_name("display")
            .short("d")
            .long("display")
            .help("Pass to open a window and display.")
            .takes_value(false))
        .arg(Arg::with_name("controls")
            .short("o")
            .long("controls")
            .help("List the camera controls. Does nothing if -c flag is not set.")
            .takes_value(false))
        .arg(Arg::with_name("setcontrols")
            .short("p")
            .long("setcontrols")
            .help("Set the camera controls. Takes a comma seperated list of strings (lowercase!) that is \"<KEY>:\"<VALUE>\"\" (e.g.) \"Contrast:10,Brightness:50\". Does nothing if -c flag is not set.").takes_value(true))

        .get_matches();

    // Query example
    if matches.is_present("query") {
        let backend_value = matches.value_of("query").unwrap();
        let mut use_backend = CaptureAPIBackend::Auto;
        // AUTO
        if backend_value == "AUTO" {
            use_backend = CaptureAPIBackend::Auto;
        } else if backend_value == "UVC" {
            use_backend = CaptureAPIBackend::UniversalVideoClass;
        } else if backend_value == "GST" {
            use_backend = CaptureAPIBackend::GStreamer;
        } else if backend_value == "V4L" {
            use_backend = CaptureAPIBackend::Video4Linux;
        } else if backend_value == "MSMF" {
            use_backend = CaptureAPIBackend::MediaFoundation;
        } else if backend_value == "AVF" {
            nokhwa_initialize(|x| {
                println!("{}", x);
            });
            use_backend = CaptureAPIBackend::AVFoundation;
        }

        match query_devices(use_backend) {
            Ok(devs) => {
                for (idx, camera) in devs.iter().enumerate() {
                    println!("Device at index {}: {}", idx, camera)
                }
            }
            Err(why) => {
                println!("Failed to query: {why}")
            }
        }
    }

    if matches.is_present("capture") {
        let backend_value = {
            match matches.value_of("capture-backend").unwrap() {
                "UVC" => CaptureAPIBackend::UniversalVideoClass,
                "GST" => CaptureAPIBackend::GStreamer,
                "V4L" => CaptureAPIBackend::Video4Linux,
                "OPENCV" => CaptureAPIBackend::OpenCv,
                "MSMF" => CaptureAPIBackend::MediaFoundation,
                "AVF" => CaptureAPIBackend::AVFoundation,
                _ => CaptureAPIBackend::Auto,
            }
        };
        let width = matches
            .value_of("width")
            .unwrap()
            .trim()
            .parse::<u32>()
            .expect("Width must be a u32!");
        let height = matches
            .value_of("height")
            .unwrap()
            .trim()
            .parse::<u32>()
            .expect("Height must be a u32!");
        let fps = matches
            .value_of("framerate")
            .unwrap()
            .trim()
            .parse::<u32>()
            .expect("Framerate must be a u32!");
        let format = {
            match matches.value_of("format").unwrap() {
                "YUYV" => FrameFormat::YUYV,
                _ => FrameFormat::MJPEG,
            }
        };

        let matches_clone = matches.clone();

        let (send, recv) = flume::unbounded();
        // spawn a thread for capture
        std::thread::spawn(move || {
            // Index camera
            if let Ok(index) = matches_clone
                .value_of("capture")
                .unwrap()
                .trim()
                .parse::<usize>()
            {
                let mut camera =
                    Camera::new_with(index, width, height, fps, format, backend_value).unwrap();

                if matches_clone.is_present("query-device") {
                    match camera.compatible_fourcc() {
                        Ok(fcc) => {
                            for ff in fcc {
                                match camera.compatible_list_by_resolution(ff) {
                                    Ok(compat) => {
                                        println!("For FourCC {}", ff);
                                        for (res, fps) in compat {
                                            println!("{}x{}: {:?}", res.width(), res.height(), fps);
                                        }
                                    }
                                    Err(why) => {
                                        println!("Failed to get compatible resolution/FPS list for FrameFormat {ff}: {why}")
                                    }
                                }
                            }
                        }
                        Err(why) => {
                            println!("Failed to get compatible FourCC: {why}")
                        }
                    }
                }

                if matches_clone.is_present("controls") {
                    match camera.camera_controls() {
                        Ok(controls) => {
                            println!("Supported Camera Controls: ");
                            for (index, control) in controls.into_iter().enumerate() {
                                println!("{}. {}", (index + 1), control)
                            }
                        }
                        Err(why) => {
                            println!("Failed to get camera controls: {why}")
                        }
                    }
                }

                if let Some(s) = matches_clone.value_of("setcontrols") {
                    let set_ctrls = s.to_string();

                    let supported = match camera.camera_controls_string() {
                        Ok(cc) => cc,
                        Err(why) => {
                            println!("Failed to get camera controls: {why}");
                            return;
                        }
                    };

                    for control in set_ctrls.split(',') {
                        let ctrl = control
                            .replace(' ', "")
                            .split(':')
                            .map(ToString::to_string)
                            .collect::<Vec<String>>();

                        let value = ctrl[1].parse::<i32>().unwrap();

                        let mut cc = match supported.get(&ctrl[0]) {
                            Some(camc) => *camc,
                            None => {
                                return;
                            }
                        };

                        cc.set_value(value).unwrap();
                        cc.set_active(true);
                        camera.set_camera_control(cc).unwrap();
                    }
                }

                // open stream
                camera.open_stream().unwrap();
                loop {
                    if let Ok(frame) = camera.frame() {
                        println!(
                            "Captured frame {}x{} @ {}FPS size {}",
                            frame.width(),
                            frame.height(),
                            fps,
                            frame.len()
                        );
                        let _send = send.send(frame);
                    }
                }
            }
        });

        // run glium
        if matches.is_present("display") {
            let gl_event_loop = EventLoop::new();
            let window_builder = WindowBuilder::new();
            let context_builder = ContextBuilder::new().with_vsync(true);
            let gl_display = Display::new(window_builder, context_builder, &gl_event_loop).unwrap();

            implement_vertex!(Vertex, position, tex_coords);

            let vert_buffer = VertexBuffer::new(
                &gl_display,
                &[
                    Vertex {
                        position: [-1.0, -1.0],
                        tex_coords: [0.0, 0.0],
                    },
                    Vertex {
                        position: [-1.0, 1.0],
                        tex_coords: [0.0, 1.0],
                    },
                    Vertex {
                        position: [1.0, 1.0],
                        tex_coords: [1.0, 1.0],
                    },
                    Vertex {
                        position: [1.0, -1.0],
                        tex_coords: [1.0, 0.0],
                    },
                ],
            )
            .unwrap();

            let idx_buf =
                IndexBuffer::new(&gl_display, PrimitiveType::TriangleStrip, &[1_u16, 2, 0, 3])
                    .unwrap();

            let program = program!(&gl_display,
                140 => {
                    vertex: "
                #version 140
                uniform mat4 matrix;
                in vec2 position;
                in vec2 tex_coords;
                out vec2 v_tex_coords;
                void main() {
                    gl_Position = matrix * vec4(position, 0.0, 1.0);
                    v_tex_coords = tex_coords;
                }
            ",
                    outputs_srgb: true,
                    fragment: "
                #version 140
                uniform sampler2D tex;
                in vec2 v_tex_coords;
                out vec4 f_color;
                void main() {
                    f_color = texture(tex, v_tex_coords);
                }
            "
                },
            )
            .unwrap();

            // run the event loop

            gl_event_loop.run(move |event, _window, ctrl| {
                *ctrl = match event {
                    Event::MainEventsCleared => {
                        let instant = Instant::now();
                        let frame = recv.recv().unwrap();
                        let capture_elapsed = instant.elapsed().as_millis();

                        let frame_size = (frame.width(), frame.height());

                        let raw_data = RawImage2d::from_raw_rgb(frame.into_raw(), frame_size);
                        let gl_texture = Texture2d::new(&gl_display, raw_data).unwrap();

                        let uniforms = uniform! {
                            matrix: [
                                [1.0, 0.0, 0.0, 0.0],
                                [0.0, -1.0, 0.0, 0.0],
                                [0.0, 0.0, 1.0, 0.0],
                                [0.0, 0.0, 0.0, 1.0f32]
                            ],
                            tex: &gl_texture
                        };

                        let mut target = gl_display.draw();
                        target.clear_color(0.0, 0.0, 0.0, 0.0);
                        target
                            .draw(
                                &vert_buffer,
                                &idx_buf,
                                &program,
                                &uniforms,
                                &Default::default(),
                            )
                            .unwrap();
                        target.finish().unwrap();

                        println!("Took {capture_elapsed}ms to capture",);
                        ControlFlow::Poll
                    }
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => ControlFlow::Exit,
                    _ => ControlFlow::Poll,
                }
            })
        }
        // dont
        else {
            loop {
                if let Ok(frame) = recv.recv() {
                    println!(
                        "Frame width {} height {} size {}",
                        frame.width(),
                        frame.height(),
                        frame.len()
                    );
                } else {
                    println!("Thread terminated, closing!");
                    break;
                }
            }
        }
    }
}
