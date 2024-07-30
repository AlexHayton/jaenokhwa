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

use jaenokhwa_core::{
    error::NokhwaError,
    types::{ApiBackend, CameraInfo},
};

/// Gets the native [`ApiBackend`]
#[must_use]
pub fn native_api_backend() -> Option<ApiBackend> {
    match std::env::consts::OS {
        "linux" => Some(ApiBackend::Video4Linux),
        "macos" | "ios" => Some(ApiBackend::AVFoundation),
        "windows" => Some(ApiBackend::MediaFoundation),
        _ => None,
    }
}

// TODO: Update as this goes
/// Query the system for a list of available devices. Please refer to the API Backends that support `Query`) <br>
/// Usually the order goes Native -> UVC -> Gstreamer.
/// # Quirks
/// - `Media Foundation`: The names may contain invalid characters since they were converted from UTF16.
/// - `WASM`: The `misc` field contains the device ID and group ID are seperated by a space (' ')
/// # Errors
/// If you use an unsupported API (check the README or crate root for more info), incompatible backend for current platform, incompatible platform, or insufficient permissions, etc
/// this will error.
pub fn query_devices() -> Result<Vec<CameraInfo>, NokhwaError> {
    match std::env::consts::OS {
        "linux" => {
            if cfg!(feature = "input-v4l") && cfg!(target_os = "linux") {
                query_v4l()
            } else {
                dbg!("Error: No suitable Backends available. Perhaps you meant to enable one of the backends such as `input-v4l`? (Please read the docs.)");
                Err(NokhwaError::UnsupportedOperationError(ApiBackend::Auto))
            }
        }
        "windows" => {
            if cfg!(feature = "input-msmf") && cfg!(target_os = "windows") {
                query_msmf()
            } else {
                dbg!("Error: No suitable Backends available. Perhaps you meant to enable one of the backends such as `input-msmf`? (Please read the docs.)");
                Err(NokhwaError::UnsupportedOperationError(ApiBackend::Auto))
            }
        }
        "macos" => {
            if cfg!(feature = "input-avfoundation") {
                query_avfoundation()
            } else {
                dbg!("Error: No suitable Backends available. Perhaps you meant to enable one of the backends such as `input-avfoundation`? (Please read the docs.)");
                Err(NokhwaError::UnsupportedOperationError(ApiBackend::Auto))
            }
        }
        "ios" => {
            if cfg!(feature = "input-avfoundation") {
                query_avfoundation()
            } else {
                dbg!("Error: No suitable Backends available. Perhaps you meant to enable one of the backends such as `input-avfoundation`? (Please read the docs.)");
                Err(NokhwaError::UnsupportedOperationError(ApiBackend::Auto))
            }
        }
        _ => {
            dbg!("Error: No suitable Backends available. You are on an unsupported platform.");
            Err(NokhwaError::NotImplementedError("Bad Platform".to_string()))
        }
    }
}

#[cfg(all(feature = "input-v4l", target_os = "linux"))]
fn query_v4l() -> Result<Vec<CameraInfo>, NokhwaError> {
    nokhwa_bindings_linux::query()
}

#[cfg(any(not(feature = "input-v4l"), not(target_os = "linux")))]
fn query_v4l() -> Result<Vec<CameraInfo>, NokhwaError> {
    Err(NokhwaError::UnsupportedOperationError(
        ApiBackend::Video4Linux,
    ))
}

// please refer to https://docs.microsoft.com/en-us/windows/win32/medfound/enumerating-video-capture-devices
#[cfg(all(feature = "input-msmf", target_os = "windows"))]
fn query_msmf() -> Result<Vec<CameraInfo>, NokhwaError> {
    nokhwa_bindings_windows::wmf::query_media_foundation_descriptors()
}

#[cfg(any(not(feature = "input-msmf"), not(target_os = "windows")))]
fn query_msmf() -> Result<Vec<CameraInfo>, NokhwaError> {
    Err(NokhwaError::UnsupportedOperationError(
        ApiBackend::MediaFoundation,
    ))
}

#[cfg(all(
    feature = "input-avfoundation",
    any(target_os = "macos", target_os = "ios")
))]
fn query_avfoundation() -> Result<Vec<CameraInfo>, NokhwaError> {
    use jaenokhwa_bindings_macos::query_avfoundation;

    Ok(query_avfoundation()?
        .into_iter()
        .collect::<Vec<CameraInfo>>())
}

#[cfg(not(all(
    feature = "input-avfoundation",
    any(target_os = "macos", target_os = "ios")
)))]
fn query_avfoundation() -> Result<Vec<CameraInfo>, NokhwaError> {
    Err(NokhwaError::UnsupportedOperationError(
        ApiBackend::AVFoundation,
    ))
}

#[cfg(feature = "input-jscam")]
fn query_wasm() -> Result<Vec<CameraInfo>, NokhwaError> {
    use crate::js_camera::query_js_cameras;
    use wasm_rs_async_executor::single_threaded::block_on;

    block_on(query_js_cameras())
}

#[cfg(not(feature = "input-jscam"))]
fn query_wasm() -> Result<Vec<CameraInfo>, NokhwaError> {
    Err(NokhwaError::UnsupportedOperationError(ApiBackend::Browser))
}
