use four_cc::FourCC;

pub const MJPEG: FourCC = FourCC(*b"mjpg");
pub const YUYV: FourCC = FourCC(*b"yuyv");
// From https://fourcc.org/rgb.php
pub const RAWRGB: FourCC = FourCC(0x32424752u32.to_be_bytes());
pub const NV12: FourCC = FourCC(*b"nv12");
pub const UYVY: FourCC = FourCC(*b"uyvy");
pub const GRAY: FourCC = FourCC(*b"gray");
