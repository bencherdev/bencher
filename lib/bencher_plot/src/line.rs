use std::{
    io::Cursor,
    ops::{Deref, Range},
};

use bencher_json::{JsonMetricKind, JsonPerf};
use chrono::{DateTime, Utc};
use image::ImageBuffer;
use once_cell::sync::Lazy;
use ordered_float::OrderedFloat;
use plotters::{
    coord::types::RangedCoordf64,
    prelude::{
        BitMapBackend, BitMapElement, Cartesian2d, ChartBuilder, ChartContext, IntoDrawingArea,
        RangedDateTime, Rectangle, Text,
    },
    series::LineSeries,
    style::{FontFamily, RGBColor, ShapeStyle, WHITE},
};

use crate::PlotError;

const IMG_WIDTH: u32 = 1024;
const IMG_HEIGHT: u32 = 768;
const TITLE_HEIGHT: u32 = 48;
const PLOT_HEIGHT: u32 = 600;
const KEY_HEIGHT: u32 = IMG_HEIGHT - PLOT_HEIGHT;

// RGB is three units in size
// https://docs.rs/image/latest/image/struct.Rgb.html
const BUFFER_SIZE: usize = IMG_WIDTH as usize * IMG_HEIGHT as usize * 3;

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
            width: IMG_WIDTH,
            height: IMG_HEIGHT,
        }
    }
}

impl LinePlot {
    pub fn new() -> LinePlot {
        Self::default()
    }

    pub fn draw(&self, title: Option<&str>, json_perf: JsonPerf) -> Result<Vec<u8>, PlotError> {
        let mut plot_buffer = vec![0; BUFFER_SIZE];

        // Use a closure that gets immediately executed here
        // This provides early return control flow and avoids the lifetime complexity
        || -> Result<(), PlotError> {
            let root_area = BitMapBackend::with_buffer(&mut plot_buffer, (self.width, self.height))
                .into_drawing_area();
            root_area.fill(&WHITE)?;

            // Bencher Wordmark
            root_area.draw(&*WORDMARK_ELEMENT)?;

            // Split header and plot areas
            let (header, plot_area) = root_area.split_vertically(TITLE_HEIGHT);

            // Adaptive title sizing
            if let Some(title) = title {
                const MAX_LEN: usize = 28;
                let title_len = title.len();
                let size = if title_len > MAX_LEN {
                    let diff = title_len - MAX_LEN;
                    std::cmp::max(TITLE_HEIGHT - u32::try_from(diff)?, 12)
                } else {
                    TITLE_HEIGHT
                };
                header.titled(title, (FontFamily::Monospace, size))?;
            }

            // Marshal the perf data into a plot-able form
            let perf_data = PerfData::new(json_perf);

            let Some(perf_data) = perf_data else {
                  // Return an informative message if there is no perf data found
                  let _chart_context = ChartBuilder::on(&plot_area)
                  .margin_top(TITLE_HEIGHT)
                  .caption(
                      format!("No Data Found: {}", Utc::now().format("%d %b %Y %H:%M:%S")),
                      (FontFamily::Monospace, 32),
                  )
                  .build_cartesian_2d(PerfData::default_x_range(), PerfData::default_y_range())?;

                  return root_area.present().map_err(Into::into);
            };

            let lines_len = perf_data.lines.len();

            if lines_len > 10 {
                // Return an informative message if there is too much data to be shown
                let _chart_context = ChartBuilder::on(&plot_area)
                    .margin_top(TITLE_HEIGHT)
                    .caption(
                        format!(
                            "Too Many Data Sets: {lines_len} found which exceeds the max of 10"
                        ),
                        (FontFamily::Monospace, 24),
                    )
                    .build_cartesian_2d(PerfData::default_x_range(), PerfData::default_y_range())?;

                return root_area.present().map_err(Into::into);
            }

            let (plot_area, key_area) = plot_area.split_vertically(PLOT_HEIGHT);

            let mut chart_context = ChartBuilder::on(&plot_area)
                .x_label_area_size(40)
                .y_label_area_size(perf_data.y_label_area_size()?)
                .margin_left(8)
                .margin_right(32)
                .margin_bottom(8)
                .build_cartesian_2d(perf_data.x_range(), perf_data.y_range())?;

            chart_context
                .configure_mesh()
                .axis_desc_style((FontFamily::Monospace, 20))
                .x_desc("Benchmark Date and Time")
                .x_labels(5)
                .x_label_style((FontFamily::Monospace, 16))
                .x_label_formatter(&PerfData::x_label_fmt)
                .y_desc(perf_data.y_desc)
                .y_labels(5)
                .y_label_style((FontFamily::Monospace, 12))
                .y_label_formatter(&PerfData::y_label_fmt)
                .max_light_lines(4)
                .draw()?;

            const KEY_LEFT_MARGIN: usize = 48;
            const BOX_GAP: usize = 12;
            let (box_x_left, box_width, box_gap) = if lines_len > 4 {
                const MIN_GAP: usize = 4;
                let extra_lines = lines_len - 4;
                let box_x_left = std::cmp::max(MIN_GAP, KEY_LEFT_MARGIN - (extra_lines * 8));
                let box_gap = std::cmp::max(MIN_GAP, BOX_GAP - extra_lines);
                let box_gaps = lines_len * box_gap;
                let width = (usize::try_from(IMG_WIDTH)? - box_x_left - box_gaps) / lines_len;
                (box_x_left, width, box_gap)
            } else {
                (KEY_LEFT_MARGIN, 200, BOX_GAP)
            };

            const BOX_HEIGHT: i32 = 24;
            let (mut box_x_left, box_width, box_gap) = (
                i32::try_from(box_x_left)?,
                i32::try_from(box_width)?,
                i32::try_from(box_gap)?,
            );

            for LineData { data, color } in perf_data.lines {
                let _series = chart_context.draw_series(LineSeries::new(
                    data.into_iter().map(|(x, y)| (x, y.into())),
                    color,
                ))?;

                let box_x_right = box_x_left + box_width;
                let points = [(box_x_left, 0), (box_x_right, BOX_HEIGHT)];
                let shape_style = ShapeStyle::from(color).filled();
                key_area.draw(&Rectangle::new(points, shape_style))?;
                box_x_left = box_x_right + box_gap;
            }

            root_area.present().map_err(Into::into)
        }()?;

        let image_buffer: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_vec(self.width, self.height, plot_buffer)
                .ok_or(PlotError::ImageBuffer)?;
        let mut image_cursor = Cursor::new(Vec::with_capacity(BUFFER_SIZE));
        image_buffer.write_to(&mut image_cursor, image::ImageOutputFormat::Jpeg(100))?;

        Ok(image_cursor.into_inner())
    }
}

struct PerfData {
    lines: Vec<LineData>,
    x: (DateTime<Utc>, DateTime<Utc>),
    y: (OrderedFloat<f64>, OrderedFloat<f64>),
    y_desc: String,
}

struct LineData {
    data: Vec<(DateTime<Utc>, OrderedFloat<f64>)>,
    color: RGBColor,
}

impl PerfData {
    fn new(json_perf: JsonPerf) -> Option<PerfData> {
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
            (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) => {
                let y_desc = format!(
                    "{}: {}",
                    json_perf.metric_kind.name, json_perf.metric_kind.units
                );
                Some(PerfData {
                    lines,
                    x: (min_x, max_x),
                    y: (min_y, max_y),
                    y_desc,
                })
            },
            _ => None,
        }
    }

    fn x_range(&self) -> Range<DateTime<Utc>> {
        self.x.0..self.x.1
    }

    fn default_x_range() -> Range<DateTime<Utc>> {
        let epoch = DateTime::default();
        epoch..epoch
    }

    fn x_label_fmt(x: &DateTime<Utc>) -> String {
        format!("{}", x.format("%d %b %Y"))
    }

    fn y_range(&self) -> Range<f64> {
        let (min, max) = if self.y.1 < OrderedFloat::from(1.0) {
            (self.y.0, self.y.1)
        } else {
            let diff = self.y.1 - self.y.0;
            (
                std::cmp::max(self.y.0 - (diff * 0.08), OrderedFloat::from(0.0)),
                (self.y.1 + (diff * 0.04)),
            )
        };
        min.into()..max.into()
    }

    fn default_y_range() -> Range<f64> {
        0.0..0.0
    }

    fn y_label_area_size(&self) -> Result<u32, PlotError> {
        let buffer = if self.y.1 < OrderedFloat::from(1.0) {
            48
        } else if self.y.1 < OrderedFloat::from(1_000.0) {
            36
        } else {
            30
        };
        let y_len =
            buffer + 6 * std::cmp::max(Self::float_len(self.y.0), Self::float_len(self.y.1));
        u32::try_from(y_len).map_err(Into::into)
    }

    fn y_label_fmt(y: &f64) -> String {
        if *y < 1.0 {
            y.to_string()
        } else {
            Self::comma_format(*y as u64)
        }
    }

    fn float_len(y: OrderedFloat<f64>) -> usize {
        let y = f64::from(y);
        if y < 1.0 {
            y.to_string().len()
        } else {
            Self::comma_format(y as u64).len()
        }
    }

    fn comma_format(y: u64) -> String {
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
        let plot_buffer = plot
            .draw(Some("Benchmark Adapter Comparison"), JSON_PERF.clone())
            .unwrap();
        save_jpeg(&plot_buffer, "perf");
    }

    #[test]
    fn test_plot_empty() {
        let plot = LinePlot::new();
        let plot_buffer = plot
            .draw(Some("Adapter Comparison"), JsonPerf::default())
            .unwrap();
        save_jpeg(&plot_buffer, "empty");
    }
}
