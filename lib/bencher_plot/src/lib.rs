use std::io::Cursor;

use image::ImageBuffer;
use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea},
    style::WHITE,
};

mod error;

pub use error::PlotError;

pub async fn plot(width: u32, height: u32) -> Result<Vec<u8>, PlotError> {
    let mut plot_buffer = vec![0; 3 * usize::try_from(width)? * usize::try_from(height)?];
    {
        let root_area =
            BitMapBackend::with_buffer(&mut plot_buffer, (width, height)).into_drawing_area();

        root_area
            .fill(&WHITE)
            .map_err(|e| PlotError::Plotters(e.to_string()))?;

        root_area
            .present()
            .map_err(|e| PlotError::Plotters(e.to_string()))?;
    }

    let image_buffer: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_vec(width, height, plot_buffer).ok_or(PlotError::ImageBuffer)?;

    let mut image_cursor = Cursor::new(Vec::with_capacity(
        3 * usize::try_from(width)? * usize::try_from(height)?,
    ));

    image_buffer.write_to(&mut image_cursor, image::ImageOutputFormat::Jpeg(100))?;

    Ok(image_cursor.into_inner())
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
