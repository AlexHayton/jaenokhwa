use four_cc::FourCC;

pub const YUV420: FourCC = FourCC(*b"420v");
pub const MJPEG: FourCC = FourCC(*b"MJPG");
pub const YUYV: FourCC = FourCC(*b"YUYV");
pub const RAWRGB: FourCC = FourCC(*b"RGB3");
pub const NV12: FourCC = FourCC(*b"nv12");
pub const UYVY: FourCC = FourCC(*b"uyvy");
// Also known as 2vuy
pub const UYVY_APPLE: FourCC = FourCC(*b"2vuy");
pub const GRAY: FourCC = FourCC(*b"GRAY");
