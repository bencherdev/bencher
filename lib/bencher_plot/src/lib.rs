use std::io::Cursor;

use image::ImageBuffer;
use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea},
    style::WHITE,
};

mod error;

pub use error::PlotError;

pub async fn plot(width: u32, height: u32) -> Result<Vec<u8>, PlotError> {
    let buffer_size = buffer_size(width, height)?;

    let mut plot_buffer = vec![0; buffer_size];
    draw(width, height, &mut plot_buffer).await?;

    let image_buffer: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_vec(width, height, plot_buffer).ok_or(PlotError::ImageBuffer)?;
    let mut image_cursor = Cursor::new(Vec::with_capacity(buffer_size));
    image_buffer.write_to(&mut image_cursor, image::ImageOutputFormat::Jpeg(100))?;

    Ok(image_cursor.into_inner())
}

async fn draw(width: u32, height: u32, plot_buffer: &mut [u8]) -> Result<(), PlotError> {
    let root_area = BitMapBackend::with_buffer(plot_buffer, (width, height)).into_drawing_area();

    root_area.fill(&WHITE)?;

    root_area.present()?;

    Ok(())
}

// RGB is three units in size
// https://docs.rs/image/latest/image/struct.Rgb.html
fn buffer_size(width: u32, height: u32) -> Result<usize, PlotError> {
    Ok(usize::try_from(width)? * usize::try_from(height)? * 3)
}

#[cfg(test)]
mod test {
    use tokio::{fs::File, io::AsyncWriteExt};

    use super::plot;

    async fn save_jpeg(jpeg: &[u8]) {
        let mut file = File::create("perf.jpeg").await.unwrap();
        file.write_all(jpeg).await.unwrap();
    }

    #[tokio::test]
    async fn test_plot() {
        let plot_buffer = plot(1024, 768).await.unwrap();
        save_jpeg(&plot_buffer).await;
    }
}
