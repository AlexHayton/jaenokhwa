use ffmpeg_next::{
    format::Pixel,
    frame::Video,
    software::scaling::{Context, Flags},
};
use jaenokhwa_core::buffer::FrameBuffer;
use jaenokhwa_core::pixel_format::{UYVY_APPLE, YUV420};

pub trait ConvertToRgb {
    fn convert_to_rgb(&self, _output_format: Pixel) -> Vec<u8> {
        todo!()
    }
}

impl ConvertToRgb for FrameBuffer {
    fn convert_to_rgb(&self, output_format: Pixel) -> Vec<u8> {
        let pixel_format = match self.source_frame_format() {
            YUV420 => Pixel::YUV420P,
            UYVY_APPLE => Pixel::UYVY422,
            _ => panic!("Unsupported pixel format {}", self.source_frame_format()),
        };

        let scaler = Context::get(
            pixel_format,
            self.width(),
            self.height(),
            output_format,
            self.width(),
            self.height(),
            Flags::BILINEAR,
        );
        match scaler {
            Ok(mut scaler) => {
                let buffer = self.buffer();
                let width = self.width() as usize;
                let height = self.height() as usize;

                // let mut output_buffer = vec![0u8; self.width() as usize * self.height() as usize * 4];
                let mut input_buffer = Video::new(pixel_format, self.width(), self.height());

                // Calculate expected buffer size (4 bytes per 2 pixels)
                let expected_size = width * height * 2;

                // Check if the buffer size matches the expected size
                assert!(
                    buffer.len() != expected_size,
                    "Buffer size does not match expected size of {}... It is {}",
                    expected_size,
                    buffer.len(),
                );

                // Copy the buffer directly into the Video object
                input_buffer.data_mut(0).copy_from_slice(buffer);

                let mut output_buffer = Video::new(output_format, self.width(), self.height());
                scaler.run(&input_buffer, &mut output_buffer).unwrap();

                return output_buffer.data(0).to_vec();
            }
            Err(e) => {
                panic!("Error creating scaler: {e}");
            }
        }
    }
}
