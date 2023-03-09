use std::{io::Cursor, ops::Range};

use bencher_json::JsonPerf;
use chrono::{DateTime, Utc};
use image::{DynamicImage, ImageBuffer};
use once_cell::sync::Lazy;
use ordered_float::OrderedFloat;
use plotters::{
    coord::types::{RangedCoordf64, RangedCoordi32},
    prelude::{
        BitMapBackend, BitMapElement, Cartesian2d, ChartBuilder, ChartContext, Circle,
        DiscreteRanged, EmptyElement, IntoDrawingArea, IntoLinspace, PathElement, RangedDateTime,
        SeriesLabelPosition, Text,
    },
    series::{LineSeries, PointSeries},
    style::{RGBColor, ShapeStyle, BLACK, BLUE, RED, WHITE},
};

mod error;

pub use error::PlotError;

const PLOT_WIDTH: u32 = 1024;
const PLOT_HEIGHT: u32 = 768;

// RGB is three units in size
// https://docs.rs/image/latest/image/struct.Rgb.html
const BUFFER_SIZE: usize = PLOT_WIDTH as usize * PLOT_HEIGHT as usize * 3;

pub const BENCHER_WORDMARK: &[u8; 4910] = include_bytes!("../wordmark.png");
#[allow(clippy::expect_used)]
static WORDMARK_ELEMENT: Lazy<BitMapElement<(i32, i32)>> = Lazy::new(|| {
    let wordmark_cursor = Cursor::new(BENCHER_WORDMARK);
    let wordmark_image =
        image::load(wordmark_cursor, image::ImageFormat::Png).expect("Failed to load wordmark");
    ((0, 5), wordmark_image).into()
});

// https://observablehq.com/@d3/color-schemes
// ["#4e79a7","#f28e2c","#e15759","#76b7b2","#59a14f","#edc949","#af7aa1","#ff9da7","#9c755f","#bab0ab"]
const TABLEAU_10: [(u8, u8, u8); 10] = [
    // #4e79a7
    (78, 121, 167),
    // #f28e2c
    (242, 142, 44),
    // #e15759
    (225, 87, 89),
    // #76b7b2
    (118, 183, 178),
    // #59a14f
    (89, 161, 79),
    // #edc949
    (237, 201, 73),
    // #af7aa1
    (175, 122, 161),
    // #ff9da7
    (255, 157, 167),
    // #9c755f
    (156, 117, 95),
    // #bab0ab
    (186, 176, 171),
];
static TABLEAU_10_RGB: Lazy<[RGBColor; 10]> = Lazy::new(|| {
    TABLEAU_10
        .into_iter()
        .map(|(r, g, b)| RGBColor(r, g, b))
        .collect::<Vec<_>>()
        .try_into()
        .expect("Failed to map Tableau 10 RGB values")
});

pub struct LinePlot {
    width: u32,
    height: u32,
}

impl Default for LinePlot {
    fn default() -> Self {
        Self {
            width: PLOT_WIDTH,
            height: PLOT_HEIGHT,
        }
    }
}

impl LinePlot {
    pub fn new() -> LinePlot {
        Self::default()
    }

    pub fn draw(&self, title: Option<&str>, json_perf: &JsonPerf) -> Result<Vec<u8>, PlotError> {
        let mut plot_buffer = vec![0; BUFFER_SIZE];
        {
            let mut root_area =
                BitMapBackend::with_buffer(&mut plot_buffer, (self.width, self.height))
                    .into_drawing_area();
            root_area.fill(&WHITE)?;

            // Bencher Wordmark
            root_area.draw(&*WORDMARK_ELEMENT)?;

            // Chart plot
            if let Some(title) = title {
                root_area = root_area.titled(title, ("sans-serif", 50))?;
            }

            let (_header, plot_area) = root_area.split_vertically(42);

            let perf_data = PerfData::new(json_perf);

            if let Some(perf_data) = perf_data {
                let mut chart_context = ChartBuilder::on(&plot_area)
                    .x_label_area_size(100)
                    .y_label_area_size(perf_data.y_label_area_size()?)
                    .margin_right(40)
                    .build_cartesian_2d(perf_data.x(), perf_data.y())?;

                chart_context
                    .configure_mesh()
                    .x_labels(5)
                    .x_label_formatter(&PerfData::x_label_fmt)
                    .y_labels(5)
                    .y_label_formatter(&PerfData::y_label_fmt)
                    .max_light_lines(4)
                    .draw()?;

                for LineData { data, color } in perf_data.lines {
                    let _series = chart_context.draw_series(LineSeries::new(
                        data.into_iter().map(|(x, y)| (x, y.into())),
                        color,
                    ))?;
                }
            } else {
                let _chart_context = ChartBuilder::on(&plot_area)
                    .caption(format!("No Data Found: {}", Utc::now()), ("sans-serif", 30))
                    .build_cartesian_2d(PerfData::default_x(), PerfData::default_y())?;
            };

            root_area.present()?;
        }

        let image_buffer: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_vec(self.width, self.height, plot_buffer)
                .ok_or(PlotError::ImageBuffer)?;
        let mut image_cursor = Cursor::new(Vec::with_capacity(BUFFER_SIZE));
        image_buffer.write_to(&mut image_cursor, image::ImageOutputFormat::Jpeg(100))?;

        Ok(image_cursor.into_inner())
    }
}

type PerfChartContext<'a> =
    ChartContext<'a, BitMapBackend<'a>, Cartesian2d<RangedDateTime<DateTime<Utc>>, RangedCoordf64>>;

struct PerfData {
    lines: Vec<LineData>,
    x: (DateTime<Utc>, DateTime<Utc>),
    y: (OrderedFloat<f64>, OrderedFloat<f64>),
}

struct LineData {
    data: Vec<(DateTime<Utc>, OrderedFloat<f64>)>,
    color: RGBColor,
}

impl PerfData {
    fn new(json_perf: &JsonPerf) -> Option<PerfData> {
        let mut min_x = None;
        let mut max_x = None;
        let mut min_y = None;
        let mut max_y = None;

        let lines = json_perf
            .results
            .iter()
            .enumerate()
            .map(|(index, result)| {
                let data = result
                    .metrics
                    .iter()
                    .map(|metric| {
                        let x_value = metric.start_time;
                        min_x = min_x
                            .map(|min| std::cmp::min(min, x_value))
                            .or(Some(x_value));
                        max_x = max_x
                            .map(|max| std::cmp::max(max, x_value))
                            .or(Some(x_value));
                        let y_value = metric.metric.value;
                        min_y = min_y
                            .map(|min| std::cmp::min(min, y_value))
                            .or(Some(y_value));
                        max_y = max_y
                            .map(|max| std::cmp::max(max, y_value))
                            .or(Some(y_value));
                        (x_value, y_value)
                    })
                    .collect();
                let color = TABLEAU_10_RGB[index % 10];
                LineData { data, color }
            })
            .collect();

        match (min_x, max_x, min_y, max_y) {
            (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) => Some(PerfData {
                lines,
                x: (min_x, max_x),
                y: (min_y, max_y),
            }),
            _ => None,
        }
    }

    fn x(&self) -> Range<DateTime<Utc>> {
        self.x.0..self.x.1
    }

    fn default_x() -> Range<DateTime<Utc>> {
        let epoch = DateTime::default();
        epoch..epoch
    }

    fn x_label_fmt(x: &DateTime<Utc>) -> String {
        format!("{}", x.format("%d %b %Y %H:%M"))
    }

    fn y(&self) -> Range<f64> {
        let diff = self.y.1 - self.y.0;
        let min = std::cmp::max(self.y.0 - (diff * 0.08), OrderedFloat::from(0.0)).into();
        let max = (self.y.1 + (diff * 0.04)).into();
        min..max
    }

    fn default_y() -> Range<f64> {
        0.0..0.0
    }

    fn y_label_area_size(&self) -> Result<u32, PlotError> {
        u32::try_from(std::cmp::max(self.y.0.to_string().len(), self.y.1.to_string().len()) * 8 + 8)
            .map_err(Into::into)
    }

    fn y_label_fmt(y: &f64) -> String {
        y.to_string()
            .as_bytes()
            .rchunks(3)
            .rev()
            .filter_map(|thousand| std::str::from_utf8(thousand).ok())
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
mod test {
    use std::{fs::File, io::Write};

    use bencher_json::JsonPerf;
    use once_cell::sync::Lazy;

    use crate::LinePlot;

    pub const PERF_DOT_JSON: &str = include_str!("../perf.json");
    static JSON_PERF: Lazy<JsonPerf> =
        Lazy::new(|| serde_json::from_str(PERF_DOT_JSON).expect("Failed to serialize perf JSON"));

    fn save_jpeg(jpeg: &[u8], name: &str) {
        let mut file = File::create(format!("{name}.jpeg")).unwrap();
        file.write_all(jpeg).unwrap();
    }

    #[test]
    fn test_plot() {
        let plot = LinePlot::new();
        let plot_buffer = plot.draw(Some("Adapter Comparison"), &JSON_PERF).unwrap();
        save_jpeg(&plot_buffer, "perf");
    }

    #[test]
    fn test_plot_empty() {
        let plot = LinePlot::new();
        let plot_buffer = plot
            .draw(Some("Adapter Comparison"), &JsonPerf::default())
            .unwrap();
        save_jpeg(&plot_buffer, "empty");
    }
}
