[![cargo version](https://img.shields.io/crates/v/jaenokhwa.svg)](https://crates.io/crates/jaenokhwa) [![docs.rs version](https://img.shields.io/docsrs/jaenokhwa)](https://docs.rs/jaenokhwa/latest/jaenokhwa/)
# Jaenokhwa
Jaenokhwa(녹화): Korean word meaning "to record, again".

An easy-to-use, cross-platform Rust webcam capture library.

## Using Jaenokhwa
Jaenokhwa can be added to your crate by adding it to your `Cargo.toml`:
```toml
[dependencies.jaenokhwa]
version = "0.11.0"
# By default we assume you want to use the native input backends and also include tools to convert various formats to RGB.
# Other functionality is gated behind the following features:

!Todo
```

Most likely, you will only use functionality provided by the `Camera` struct. If you need lower-level access, you may instead opt to use the raw capture backends found at `jaenokhwa::backends::capture::*`.

## Example
```rust
let cameras = query_devices();
let index = CameraIndex::Index(cameras[0].unique_id()); 
// Use the string to specify a unique camera ID


// request the absolute highest resolution CameraFormat that can be decoded to RGB.
let requested = RequestedFormat::<RgbFormat>::new(RequestedFormatType::AbsoluteHighestFrameRate);
// make the camera
let mut camera = Camera::new(index, requested).unwrap();

// get a frame
let frame = camera.frame().unwrap();
println!("Captured Single Frame of {}", frame.buffer().len());
// decode into an ImageBuffer
let decoded = frame.decode_image::<RgbFormat>().unwrap();
println!("Decoded Frame of {}", decoded.len());
```

A command line app made with `jaenokhwa` can be found in the `examples` folder.

## API Support
The table below lists current `jaenokhwa` API support.
- The `Backend` column signifies the backend.
- The `Input` column signifies reading frames from the camera
- The `Query` column signifies system device list support
- The `Query-Device` column signifies reading device capabilities
- The `Platform` column signifies what Platform this is availible on.

 | Backend                              | Input              | Query             | Query-Device       | Platform            |
 |-----------------------------------------|-------------------|--------------------|-------------------|--------------------|
 | Video4Linux(`input-native`)          | ✅                 | ✅                 | ✅                | Linux               |
 | MSMF(`input-native`)                 | ✅                 | ✅                 | ✅                | Windows             |
 | AVFoundation(`input-native`)   | ✅                 | ✅                 | ✅                | Mac                 |
 | WASM(`input-wasm`)                | 🚧                 | 🚧                 | 🚧                | Browser(Web)        |

 ✅: Working, 🔮 : Experimental, ❌ : Not Supported, 🚧: Planned/WIP

  ^ = May be bugged. Also supports IP Cameras. 

## Feature
The default feature includes nothing. Anything starting with `input-*` is a feature that enables the specific backend. 

`input-*` features:
 - `input-native`: Uses either V4L2(Linux), MSMF(Windows), or AVFoundation(Mac OS)
 - `input-jscam`: Enables the use of the `JSCamera` struct, which uses browser APIs. (Web)

Conversely, anything that starts with `output-*` controls a feature that controls the output of something (usually a frame from the camera)

`output-*` features:
 - `output-threaded`: Enable the threaded/callback based camera. 

Other features:
 - `docs-only`: Documentation feature. Enabled for docs.rs builds.
 - `docs-nolink`: Build documentation **without** linking to any libraries. Enabled for docs.rs builds.
 - `test-fail-warning`: Fails on warning. Enabled in CI.

You many want to pick and choose to reduce bloat.

## Issues
If you come across a colour format / FourCC we can't handle yet, let us know! I'm sure we can work on it.
If you are making an issue, please make sure that
 - It has not been made yet
 - Attach what you were doing, your environment, steps to reproduce, and backtrace.
Thank you!

## Contributing
Contributions are welcome!
 - Please `rustfmt` all your code and adhere to the clippy lints (unless necessary not to do so)
 - Please limit use of `unsafe`, use objc2 for all native callbacks.
 - All contributions are under the Apache 2.0 license unless otherwise specified

## Minimum Service Rust Version
`jaenokhwa` may build on older versions of `rustc`, but there is no guarantee except for the latest stable rust. 

## Sponsors
- $40/mo sponsors:
  - [erlend-sh](https://github.com/erlend-sh)
  - [DanielMSchmidt](https://github.com/DanielMSchmidt)
- $5/mo sponsors:
  - [remifluff](https://github.com/remifluff)
  - [gennyble](https://github.com/gennyble)
  
Please consider [donating](https://buymeacoffee.com/alexhaytong)! Every little helps ❤️
