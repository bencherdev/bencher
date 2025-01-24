use std::sync::LazyLock;
use std::{io::Cursor, ops::Range};

use bencher_json::{project::perf::JsonPerfMetrics, JsonPerf};
use bencher_json::{JsonMeasure, Units};
use chrono::{DateTime, Duration, Utc};
use image::{GenericImageView, ImageBuffer};
use ordered_float::OrderedFloat;
use plotters::{
    coord::{types::RangedCoordf64, Shift},
    prelude::{
        BitMapBackend, BitMapElement, ChartBuilder, DrawingArea, IntoDrawingArea, MultiLineText,
        Ranged, Rectangle,
    },
    series::LineSeries,
    style::{Color, FontFamily, RGBColor, ShapeStyle, WHITE},
};

use crate::PlotError;

const IMG_WIDTH: u32 = 1024;
const IMG_HEIGHT: u32 = 768;
const TITLE_HEIGHT: u32 = 48;
const PLOT_HEIGHT: u32 = 600;
const KEY_HEIGHT: u32 = IMG_HEIGHT - PLOT_HEIGHT;

const MAX_TITLE_LEN: usize = 28;
const X_LABELS: i64 = 5;
const Y_LABELS: usize = 5;
const DATE_TIME_FMT: &str = "%d %b %Y %H:%M:%S";

// RGB is three units in size
// https://docs.rs/image/latest/image/struct.Rgb.html
const BUFFER_SIZE: usize = IMG_WIDTH as usize * IMG_HEIGHT as usize * 3;

const MAX_LINES: usize = 8;

pub const BENCHER_WORDMARK: &[u8; 4910] = include_bytes!("../wordmark.png");
#[allow(clippy::expect_used)]
static WORDMARK_ELEMENT: LazyLock<BitMapElement<(i32, i32)>> = LazyLock::new(|| {
    let wordmark_cursor = Cursor::new(BENCHER_WORDMARK);
    let wordmark_image =
        image::load(wordmark_cursor, image::ImageFormat::Png).expect("Failed to load wordmark");
    let size = wordmark_image.dimensions();
    let buf = wordmark_image.to_rgb8().into_raw();
    BitMapElement::with_owned_buffer((0, 5), size, buf).expect("Failed to create wordmark")
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

    pub fn draw(&self, title: Option<&str>, json_perf: &JsonPerf) -> Result<Vec<u8>, PlotError> {
        let mut plot_buffer = vec![0; BUFFER_SIZE];
        self.draw_inner(title, json_perf, &mut plot_buffer)?;

        let image_buffer: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_vec(self.width, self.height, plot_buffer)
                .ok_or(PlotError::ImageBuffer)?;
        let mut image_cursor = Cursor::new(Vec::with_capacity(BUFFER_SIZE));
        image_buffer.write_to(&mut image_cursor, image::ImageFormat::Jpeg)?;

        Ok(image_cursor.into_inner())
    }

    #[allow(clippy::too_many_lines)]
    fn draw_inner(
        &self,
        title: Option<&str>,
        json_perf: &JsonPerf,
        plot_buffer: &mut [u8],
    ) -> Result<(), PlotError> {
        let root_area =
            BitMapBackend::with_buffer(plot_buffer, (self.width, self.height)).into_drawing_area();
        let (header, plot_area) = Self::split_header(&root_area)?;
        Self::header(title, json_perf, &header)?;
        // Marshal the perf data into a plot-able form
        let perf_data = PerfData::new(json_perf);
        let Some(perf_data) = perf_data else {
            return Self::no_data_found(&root_area, &plot_area);
        };
        Self::chart(perf_data, &plot_area)?;
        root_area.present().map_err(Into::into)
    }

    fn split_header<'b>(
        root_area: &DrawingArea<BitMapBackend<'b>, Shift>,
    ) -> Result<(Area<'b>, Area<'b>), PlotError> {
        root_area.fill(&WHITE)?;
        // Bencher Wordmark
        root_area.draw(&*WORDMARK_ELEMENT)?;
        Ok(root_area.split_vertically(TITLE_HEIGHT))
    }

    fn header(
        title: Option<&str>,
        json_perf: &JsonPerf,
        header: &Area<'_>,
    ) -> Result<(), PlotError> {
        // Adaptive title sizing
        let title = title.unwrap_or(json_perf.project.name.as_ref());
        let title_len = title.len();
        let size = if title_len > MAX_TITLE_LEN {
            let diff = title_len - MAX_TITLE_LEN;
            std::cmp::max(TITLE_HEIGHT - u32::try_from(diff)?, 12)
        } else {
            TITLE_HEIGHT
        };
        header.titled(title, (FontFamily::Monospace, size))?;
        Ok(())
    }

    fn no_data_found(root_area: &Area<'_>, plot_area: &Area<'_>) -> Result<(), PlotError> {
        // Return an informative message if there is no perf data found
        let _chart_context = ChartBuilder::on(plot_area)
            .margin_top(TITLE_HEIGHT)
            .caption(
                format!("No Data Found: {}", Utc::now().format(DATE_TIME_FMT)),
                (FontFamily::Monospace, 32),
            )
            .build_cartesian_2d(PerfData::default_x_range(), PerfData::default_y_range())?;

        root_area.present().map_err(Into::into)
    }

    #[allow(clippy::too_many_lines)]
    fn chart(
        perf_data: PerfData,
        plot_area: &DrawingArea<BitMapBackend<'_>, Shift>,
    ) -> Result<(), PlotError> {
        let (plot_area, key_area) = plot_area.split_vertically(PLOT_HEIGHT);

        let chart_context = ChartBuilder::on(&plot_area)
            .x_label_area_size(40)
            .y_label_area_size(perf_data.left_y_label_area_size()?)
            .right_y_label_area_size(perf_data.right_y_label_area_size().unwrap_or(Ok(0))?)
            .margin_left(8)
            .margin_right(32)
            .margin_bottom(8)
            .build_cartesian_2d(perf_data.x_range(), perf_data.left_y_range())?;

        let mut chart_context = if let Some(right_y_range) = perf_data.right_y_range() {
            Chart::Dual(chart_context.set_secondary_coord(perf_data.x_range(), right_y_range))
        } else {
            Chart::Single(chart_context)
        };

        match &mut chart_context {
            Chart::Single(chart_context) => {
                chart_context
                    .configure_mesh()
                    .axis_desc_style((FontFamily::Monospace, 20))
                    .x_desc("Benchmark Date and Time")
                    .x_labels(usize::try_from(X_LABELS)?)
                    .x_label_style((FontFamily::Monospace, 16))
                    .x_label_formatter(&|x| perf_data.x_label_fmt(x))
                    .y_desc(&perf_data.left_y_desc)
                    .y_labels(Y_LABELS)
                    .y_label_style((FontFamily::Monospace, 12))
                    .y_label_formatter(&|&y| {
                        Units::format_number(y, perf_data.trim_left_key_point_decimal())
                    })
                    .max_light_lines(4)
                    .draw()?;
            },
            Chart::Dual(chart_context) => {
                chart_context
                    .configure_mesh()
                    .axis_desc_style((FontFamily::Monospace, 20))
                    .x_desc("Benchmark Date and Time")
                    .x_labels(usize::try_from(X_LABELS)?)
                    .x_label_style((FontFamily::Monospace, 16))
                    .x_label_formatter(&|x| perf_data.x_label_fmt(x))
                    .y_desc(&perf_data.left_y_desc)
                    .y_labels(Y_LABELS)
                    .y_label_style((FontFamily::Monospace, 12))
                    .y_label_formatter(&|&y| {
                        Units::format_number(y, perf_data.trim_left_key_point_decimal())
                    })
                    .max_light_lines(4)
                    .draw()?;

                if let (Some(y_desc), Some(trim_decimal)) = (
                    &perf_data.right_y_desc,
                    perf_data.trim_right_key_point_decimal(),
                ) {
                    chart_context
                        .configure_secondary_axes()
                        .axis_desc_style((FontFamily::Monospace, 20))
                        .y_desc(y_desc)
                        .y_labels(Y_LABELS)
                        .label_style((FontFamily::Monospace, 12))
                        .y_label_formatter(&|&y| Units::format_number(y, trim_decimal))
                        .draw()?;
                }
            },
        }

        let plot_box = perf_data.plot_box()?;
        let mut box_x_left = plot_box.x_left;
        for LineData {
            data,
            anchor,
            color,
            dimensions,
        } in perf_data.lines
        {
            match &mut chart_context {
                Chart::Single(chart_context) => {
                    let _series = chart_context.draw_series(
                        LineSeries::new(
                            data.into_iter().map(|(x, y)| (x, y.into())),
                            color.filled(),
                        )
                        .point_size(2),
                    )?;
                },
                Chart::Dual(chart_context) => match anchor {
                    Anchor::Left => {
                        let _series = chart_context.draw_series(
                            LineSeries::new(
                                data.into_iter().map(|(x, y)| (x, y.into())),
                                color.filled(),
                            )
                            .point_size(2),
                        )?;
                    },
                    Anchor::Right => {
                        let _series = chart_context.draw_secondary_series(
                            LineSeries::new(
                                data.into_iter().map(|(x, y)| (x, y.into())),
                                color.filled(),
                            )
                            .point_size(2),
                        )?;
                    },
                },
            }

            let box_x_right = box_x_left + plot_box.width;

            let points = [(box_x_left, 0), (box_x_right, plot_box.height)];
            let shape_style = ShapeStyle::from(color).filled();
            let rectangle = Rectangle::new(points, shape_style);
            key_area.draw(&rectangle)?;

            let mut font = 16;
            let text = loop {
                let text = MultiLineText::from_str(
                    dimensions.as_str(),
                    (box_x_left, plot_box.text_start),
                    (FontFamily::Monospace, font),
                    plot_box.text_width,
                );
                let (_, text_height) = text.estimate_dimension().map_err(PlotError::Font)?;
                if text_height < plot_box.text_end || font == 8 {
                    break text;
                }
                font -= 1;
            };
            key_area.draw(&text)?;

            box_x_left = box_x_right + plot_box.gap;
        }

        Ok(())
    }
}

// https://github.com/plotters-rs/plotters/blob/v0.3.7/plotters/examples/two-scales.rs
enum Chart<'b> {
    Single(
        plotters::chart::ChartContext<
            'b,
            plotters_bitmap::BitMapBackend<'b>,
            plotters::prelude::Cartesian2d<
                plotters::prelude::RangedDateTime<chrono::DateTime<chrono::Utc>>,
                plotters::coord::types::RangedCoordf64,
            >,
        >,
    ),
    Dual(
        plotters::chart::DualCoordChartContext<
            'b,
            plotters_bitmap::BitMapBackend<'b>,
            plotters::prelude::Cartesian2d<
                plotters::prelude::RangedDateTime<chrono::DateTime<chrono::Utc>>,
                plotters::coord::types::RangedCoordf64,
            >,
            plotters::prelude::Cartesian2d<
                plotters::prelude::RangedDateTime<chrono::DateTime<chrono::Utc>>,
                plotters::coord::types::RangedCoordf64,
            >,
        >,
    ),
}

type Area<'b> = DrawingArea<BitMapBackend<'b>, Shift>;

struct PerfData {
    lines: Vec<LineData>,
    x: (DateTime<Utc>, DateTime<Utc>),
    x_time: bool,
    left_y: (OrderedFloat<f64>, OrderedFloat<f64>),
    left_y_desc: String,
    right_y: Option<(OrderedFloat<f64>, OrderedFloat<f64>)>,
    right_y_desc: Option<String>,
}

struct LineData {
    data: Vec<(DateTime<Utc>, OrderedFloat<f64>)>,
    anchor: Anchor,
    color: RGBColor,
    dimensions: String,
}

#[derive(Clone, Copy, Default)]
enum Anchor {
    #[default]
    Left,
    Right,
}

impl PerfData {
    #[allow(clippy::too_many_lines)]
    fn new(json_perf: &JsonPerf) -> Option<PerfData> {
        let mut json_measures: Vec<&JsonMeasure> = Vec::with_capacity(2);
        for result in &json_perf.results {
            if !json_measures
                .iter()
                .any(|measure| measure.uuid == result.measure.uuid)
            {
                json_measures.push(&result.measure);
            }
        }
        let left_measure = json_measures.first()?;
        let right_measure = json_measures.get(1);

        let anchor = |measure: &JsonMeasure| -> Option<Anchor> {
            if measure.uuid == left_measure.uuid {
                Some(Anchor::Left)
            } else if let Some(right_measure) = right_measure {
                (measure.uuid == right_measure.uuid).then_some(Anchor::Right)
            } else {
                None
            }
        };

        // There needs to be at least one measure
        // let json_measure = json_perf.results.first().map(|result| &result.measure)?;

        let mut min_x = None;
        let mut max_x = None;

        let mut left_min_y = None;
        let mut left_max_y = None;

        let mut right_min_y = None;
        let mut right_max_y = None;

        let lines = json_perf
            .results
            .iter()
            .take(MAX_LINES)
            .enumerate()
            .map(|(index, result)| {
                let anchor = anchor(&result.measure).unwrap_or_default();
                let data = result
                    .metrics
                    .iter()
                    .map(|metric| {
                        let x_value = metric.start_time.into_inner();
                        min_x = min_x
                            .map(|min| std::cmp::min(min, x_value))
                            .or(Some(x_value));
                        max_x = max_x
                            .map(|max| std::cmp::max(max, x_value))
                            .or(Some(x_value));
                        let y_value = metric.metric.value;
                        match anchor {
                            Anchor::Left => {
                                left_min_y = left_min_y
                                    .map(|min| std::cmp::min(min, y_value))
                                    .or(Some(y_value));
                                left_max_y = left_max_y
                                    .map(|max| std::cmp::max(max, y_value))
                                    .or(Some(y_value));
                            },
                            Anchor::Right => {
                                right_min_y = right_min_y
                                    .map(|min| std::cmp::min(min, y_value))
                                    .or(Some(y_value));
                                right_max_y = right_max_y
                                    .map(|max| std::cmp::max(max, y_value))
                                    .or(Some(y_value));
                            },
                        }
                        (x_value, y_value)
                    })
                    .collect();
                let color = LineData::color(index);
                let dimensions = LineData::dimensions(result);
                LineData {
                    data,
                    anchor,
                    color,
                    dimensions,
                }
            })
            .collect::<Vec<LineData>>();

        if let (Some(min_x), Some(max_x), Some(left_min_y), Some(left_max_y)) =
            (min_x, max_x, left_min_y, left_max_y)
        {
            let x = (min_x, max_x);
            let x_time = max_x - min_x < Duration::days(X_LABELS);

            fn measure_units(
                measure: &JsonMeasure,
                min_y: OrderedFloat<f64>,
            ) -> (OrderedFloat<f64>, String) {
                let units = Units::new(*min_y, measure.units.clone());
                let factor = units.scale_factor();
                let y_desc = format!("{}: {}", measure.name, units.scale_units());
                (factor, y_desc)
            }

            let (left_factor, left_y_desc) = measure_units(left_measure, left_min_y);
            let left_min_y = left_min_y / left_factor;
            let left_max_y = left_max_y / left_factor;
            let left_y = (left_min_y, left_max_y);

            let (right_factor, right_y_desc, right_y) =
                if let (Some(right_measure), Some(right_min_y), Some(right_max_y)) =
                    (right_measure, right_min_y, right_max_y)
                {
                    let (right_factor, desc) = measure_units(right_measure, right_min_y);
                    let right_min_y = right_min_y / right_factor;
                    let right_max_y = right_max_y / right_factor;
                    (right_factor, Some(desc), Some((right_min_y, right_max_y)))
                } else {
                    (1.0.into(), None, None)
                };

            let lines = lines
                .into_iter()
                .map(|line| LineData {
                    data: line
                        .data
                        .into_iter()
                        .map(|(x, y)| {
                            (
                                x,
                                y / match line.anchor {
                                    Anchor::Left => left_factor,
                                    Anchor::Right => right_factor,
                                },
                            )
                        })
                        .collect(),
                    ..line
                })
                .collect();

            Some(PerfData {
                lines,
                x,
                x_time,
                left_y,
                left_y_desc,
                right_y,
                right_y_desc,
            })
        } else {
            None
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn x_range(&self) -> Range<DateTime<Utc>> {
        let diff = Duration::seconds(((self.x.1 - self.x.0).num_seconds() as f64 * 0.04) as i64);
        self.x.0..(self.x.1 + diff)
    }

    fn default_x_range() -> Range<DateTime<Utc>> {
        let epoch = DateTime::default();
        epoch..epoch
    }

    fn x_label_fmt(&self, x: &DateTime<Utc>) -> String {
        let fmt = if self.x_time {
            DATE_TIME_FMT
        } else {
            "%d %b %Y"
        };
        format!("{}", x.format(fmt))
    }

    fn left_y_range(&self) -> Range<f64> {
        Self::y_range_inner(self.left_y)
    }

    fn right_y_range(&self) -> Option<Range<f64>> {
        self.right_y.map(Self::y_range_inner)
    }

    fn y_range_inner(y: (OrderedFloat<f64>, OrderedFloat<f64>)) -> Range<f64> {
        let diff = y.1 - y.0;
        let min = std::cmp::max(y.0 - (diff * 0.08), OrderedFloat::from(0.0));
        let max = y.1 + (diff * 0.04);
        min.into()..max.into()
    }

    fn default_y_range() -> Range<f64> {
        0.0..0.0
    }

    fn left_key_points(&self) -> Vec<f64> {
        RangedCoordf64::from(self.left_y_range()).key_points(Y_LABELS)
    }

    fn right_key_points(&self) -> Option<Vec<f64>> {
        self.right_y_range()
            .map(|range| RangedCoordf64::from(range).key_points(Y_LABELS))
    }

    fn trim_key_point_decimal(&self) -> bool {
        self.trim_left_key_point_decimal() && self.trim_right_key_point_decimal().unwrap_or(true)
    }

    fn trim_left_key_point_decimal(&self) -> bool {
        !self
            .left_key_points()
            .iter()
            .any(|y| !format!("{y:.2}").ends_with(".00"))
    }

    fn trim_right_key_point_decimal(&self) -> Option<bool> {
        self.right_key_points()
            .map(|points| !points.iter().any(|y| !format!("{y:.2}").ends_with(".00")))
    }

    fn left_y_label_area_size(&self) -> Result<u32, PlotError> {
        let y_range = self.left_key_points();
        let trim_decimal = self.trim_left_key_point_decimal();
        Self::y_label_area_size_inner(y_range, trim_decimal)
    }

    fn right_y_label_area_size(&self) -> Option<Result<u32, PlotError>> {
        let y_range = self.right_key_points()?;
        let trim_decimal = self.trim_right_key_point_decimal()?;
        Some(Self::y_label_area_size_inner(y_range, trim_decimal))
    }

    fn y_label_area_size_inner(y_range: Vec<f64>, trim_decimal: bool) -> Result<u32, PlotError> {
        let min = y_range.first().copied().unwrap_or_default();
        let max = y_range.last().copied().unwrap_or_default();
        let buffer = if max < 1.0 {
            40
        } else if max < 1_000.0 {
            36
        } else {
            32
        };
        let float_len = |y: f64| -> usize { Units::format_number(y, trim_decimal).len() };
        let y_len = buffer + 6 * std::cmp::max(float_len(min), float_len(max));
        u32::try_from(y_len).map_err(Into::into)
    }

    fn plot_box(&self) -> Result<PlotBox, PlotError> {
        const KEY_LEFT_MARGIN: usize = 48;
        const BOX_GAP: usize = 12;
        const BOX_HEIGHT: i32 = 24;
        const TEXT_START: i32 = BOX_HEIGHT + 4;

        let lines_len = self.lines.len();
        let (box_x_left, box_width, box_gap) = if lines_len > 3 {
            const MIN_GAP: usize = 4;
            let extra_lines = lines_len - 4;
            let box_x_left = std::cmp::max(MIN_GAP, KEY_LEFT_MARGIN - (extra_lines * 8));
            let box_gap = std::cmp::max(MIN_GAP, BOX_GAP - extra_lines);
            let box_gaps = lines_len * box_gap;
            #[allow(clippy::integer_division)]
            let width = (usize::try_from(IMG_WIDTH)? - box_x_left - box_gaps) / lines_len;
            (box_x_left, width, box_gap)
        } else {
            (KEY_LEFT_MARGIN, 256, BOX_GAP)
        };

        let text_end = i32::try_from(KEY_HEIGHT)? - TEXT_START - 48;
        let text_width = u32::try_from(box_width)?;

        Ok(PlotBox {
            x_left: i32::try_from(box_x_left)?,
            width: i32::try_from(box_width)?,
            height: BOX_HEIGHT,
            gap: i32::try_from(box_gap)?,
            text_start: TEXT_START,
            text_end,
            text_width,
        })
    }
}

struct PlotBox {
    x_left: i32,
    width: i32,
    height: i32,
    gap: i32,
    text_start: i32,
    text_end: i32,
    text_width: u32,
}

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
#[allow(clippy::expect_used)]
static TABLEAU_10_RGB: LazyLock<[RGBColor; 10]> = LazyLock::new(|| {
    TABLEAU_10
        .into_iter()
        .map(|(r, g, b)| RGBColor(r, g, b))
        .collect::<Vec<_>>()
        .try_into()
        .expect("Failed to map Tableau 10 RGB values")
});

impl LineData {
    #[allow(clippy::indexing_slicing)]
    fn color(index: usize) -> RGBColor {
        TABLEAU_10_RGB[index % 10]
    }

    fn dimensions(result: &JsonPerfMetrics) -> String {
        format!(
            "- {}\n- {}\n- {}\n- {}",
            result.branch.name, result.testbed.name, result.benchmark.name, result.measure.name,
        )
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod test {
    use std::{fs::File, io::Write, sync::LazyLock};

    use bencher_json::JsonPerf;

    use crate::LinePlot;

    pub const PERF_DOT_JSON: &str = include_str!("../perf.json");
    static JSON_PERF: LazyLock<JsonPerf> = LazyLock::new(|| {
        serde_json::from_str(PERF_DOT_JSON).expect("Failed to serialize perf JSON")
    });

    pub const DECIMAL_DOT_JSON: &str = include_str!("../decimal.json");
    static JSON_PERF_DECIMAL: LazyLock<JsonPerf> = LazyLock::new(|| {
        serde_json::from_str(DECIMAL_DOT_JSON).expect("Failed to serialize perf JSON")
    });

    fn save_jpeg(jpeg: &[u8], name: &str) {
        let mut file = File::create(format!("{name}.jpeg")).unwrap();
        file.write_all(jpeg).unwrap();
    }

    #[test]
    fn test_plot() {
        let plot = LinePlot::new();
        let plot_buffer = plot
            .draw(Some("Benchmark Adapter Comparison"), &JSON_PERF)
            .unwrap();
        save_jpeg(&plot_buffer, "perf");
    }

    #[test]
    fn test_plot_decimal() {
        let plot = LinePlot::new();
        let plot_buffer = plot
            .draw(Some("Benchmark Adapter Comparison"), &JSON_PERF_DECIMAL)
            .unwrap();
        save_jpeg(&plot_buffer, "decimal");
    }

    #[test]
    fn test_plot_empty() {
        let plot = LinePlot::new();
        let mut json_perf = JSON_PERF.clone();
        json_perf.results.clear();
        let plot_buffer = plot.draw(None, &json_perf).unwrap();
        save_jpeg(&plot_buffer, "empty");
    }
}
