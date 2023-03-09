use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea},
    style::WHITE,
};

mod error;

pub use error::PlotError;

pub async fn plot(length: u32, width: u32) -> Result<Vec<u8>, PlotError> {
    let mut plot_buffer = vec![0; 3 * usize::try_from(length)? * usize::try_from(width)?];
    {
        let root_area =
            BitMapBackend::with_buffer(&mut plot_buffer, (length, width)).into_drawing_area();

        root_area
            .fill(&WHITE)
            .map_err(|e| PlotError::Plotters(e.to_string()))?;

        root_area
            .present()
            .map_err(|e| PlotError::Plotters(e.to_string()))?;
    }
    Ok(plot_buffer)
}

#[cfg(test)]
mod test {
    use tokio::{fs::File, io::AsyncWriteExt};

    use super::plot;

    async fn save_jpeg(jpeg: &[u8]) {
        let mut file = File::create("perf.bmp").await.unwrap();
        file.write_all(jpeg).await.unwrap();
    }

    #[tokio::test]
    async fn test_plot() {
        let plot_buffer = plot(1024, 768).await.unwrap();
        save_jpeg(&plot_buffer).await;
    }
}
