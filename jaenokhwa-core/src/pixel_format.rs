use four_cc_nokhwa::FourCC;

pub const YUV420V: FourCC = FourCC(*b"420v");
pub const MJPEG: FourCC = FourCC(*b"mjpg");
pub const YUYV: FourCC = FourCC(*b"yuyv");
pub const RAWRGB: FourCC = FourCC(*b"rgb3");
pub const NV12: FourCC = FourCC(*b"nv12");
pub const UYVY: FourCC = FourCC(*b"uyvy");
// Also known as 2vuy
pub const UYVY_APPLE: FourCC = FourCC(*b"2vuy");
pub const GRAY: FourCC = FourCC(*b"gray");
