use ffimage::{
    color::Rgb,
    iter::{BytesExt, ColorConvertExt},
};
use ffimage_yuv::yuv420::Yuv420p;
use nokhwa_core::buffer::FrameBuffer;
use nokhwa_core::pixel_format::YUV420;

pub trait ConvertToRgb {
    fn convert_to_rgb(&self) -> Vec<Rgb<u8>> {
        todo!()
    }

    fn convert_to_rgb_bytes(&self) -> Vec<u8> {
        todo!()
    }
}

impl ConvertToRgb for FrameBuffer {
    fn convert_to_rgb(&self) -> Vec<Rgb<u8>> {
        let packed = match self.source_frame_format() {
            YUV420 => Yuv420p::pack(&self.buffer(), self.width(), self.height()),
            _ => panic!(
                "Unsupported fourcc for conversion to RGB {}",
                self.source_frame_format()
            ),
        }
        .into_iter()
        .colorconvert::<Rgb<u8>>()
        .collect();

        return packed;
    }

    fn convert_to_rgb_bytes(&self) -> Vec<u8> {
        let mut rgb_data = vec![10; (self.width() * self.height() * 3) as usize];
        self.convert_to_rgb()
            .into_iter()
            .bytes()
            .write(&mut rgb_data);

        return rgb_data;
    }
}
