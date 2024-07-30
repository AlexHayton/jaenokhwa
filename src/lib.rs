#![deny(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions)]
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
#![cfg_attr(feature = "test-fail-warning", deny(warnings))]
#![cfg_attr(feature = "docs-features", feature(doc_cfg))]
//! # jaenokhwa
//! An easy-to-use, cross-platform Rust Webcam Capture Library
//!
//! The raw backends can be found in [`backends`](crate::backends)
//!
//! The [`Camera`] struct is what you will likely use.
//!
//! The recommended default feature to enable is `input-native`. The library will not work without
//! at least one `input-*` feature enabled.
//!
//! Please read the README.md for more.

/// Raw access to each of Nokhwa's backends.
pub mod backends;
mod camera;
/// A camera that uses native browser APIs meant for WASM applications.
#[cfg(feature = "input-jscam")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-jscam")))]
pub mod js_camera;

pub mod query;
/// A camera that runs in a different thread and can call your code based on callbacks.
#[cfg(feature = "output-threaded")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-threaded")))]
pub mod threaded;

/// Convert to RGB using ffimage-yuv
#[cfg(feature = "output-convert-to-rgb")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-convert-to-rgb")))]
pub mod convert_to_rgb;

pub use camera::Camera;
pub use jaenokhwa_core::{buffer::FrameBuffer, error::NokhwaError};
pub use query::*;
#[cfg(feature = "output-threaded")]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-threaded")))]
pub use threaded::CallbackCamera;

pub mod utils {
    pub use jaenokhwa_core::types::*;
}

pub mod error {
    pub use jaenokhwa_core::error::NokhwaError;
}

pub mod camera_traits {
    pub use jaenokhwa_core::traits::*;
}

pub mod pixel_format {
    pub use jaenokhwa_core::pixel_format::*;
}

pub mod buffer {
    pub use jaenokhwa_core::buffer::*;
}
