/*
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

use crate::types::Resolution;
use bytes::Bytes;
use four_cc::FourCC;

/// A buffer returned by a camera to accommodate custom decoding.
/// Contains information of Resolution, the buffer's [`FrameFormat`], and the buffer.
///
/// Note that decoding on the main thread **will** decrease your performance and lead to dropped frames.
#[derive(Clone, Debug, Hash, PartialOrd, PartialEq, Eq)]
pub struct FrameBuffer {
    resolution: Resolution,
    buffer: Bytes,
    source_frame_format: FourCC,
}

impl FrameBuffer {
    /// Creates a new buffer with a [`&[u8]`].
    #[must_use]
    #[inline]
    pub fn new(res: Resolution, buf: &[u8], source_frame_format: FourCC) -> Self {
        Self {
            resolution: res,
            buffer: Bytes::copy_from_slice(buf),
            source_frame_format,
        }
    }

    /// Get the [`Resolution`] of this buffer.
    #[must_use]
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    #[must_use]
    /// Get the width of this buffer.
    pub fn width(&self) -> u32 {
        self.resolution.width()
    }

    #[must_use]
    /// Get the height of this buffer.
    pub fn height(&self) -> u32 {
        self.resolution.height()
    }

    /// Get the data of this buffer.
    #[must_use]
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Get a owned version of this buffer.
    #[must_use]
    pub fn buffer_bytes(&self) -> Bytes {
        self.buffer.clone()
    }

    /// Get the [`FourCC`] of this buffer.
    #[must_use]
    pub fn source_frame_format(&self) -> FourCC {
        self.source_frame_format.clone()
    }
}
