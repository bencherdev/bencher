use tabled::{settings::Style, Table};

use crate::parser::project::perf::CliPerfTableStyle;

#[derive(Debug, Clone, Copy)]
pub enum TableStyle {
    Empty,
    Blank,
    Ascii,
    AsciiRounded,
    Modern,
    Sharp,
    Rounded,
    Psql,
    Markdown,
    ReStructuredText,
    Extended,
    Dots,
}

impl From<CliPerfTableStyle> for TableStyle {
    fn from(table_style: CliPerfTableStyle) -> Self {
        match table_style {
            CliPerfTableStyle::Empty => TableStyle::Empty,
            CliPerfTableStyle::Blank => TableStyle::Blank,
            CliPerfTableStyle::Ascii => TableStyle::Ascii,
            CliPerfTableStyle::AsciiRounded => TableStyle::AsciiRounded,
            CliPerfTableStyle::Modern => TableStyle::Modern,
            CliPerfTableStyle::Sharp => TableStyle::Sharp,
            CliPerfTableStyle::Rounded => TableStyle::Rounded,
            CliPerfTableStyle::Psql => TableStyle::Psql,
            CliPerfTableStyle::Markdown => TableStyle::Markdown,
            CliPerfTableStyle::ReStructuredText => TableStyle::ReStructuredText,
            CliPerfTableStyle::Extended => TableStyle::Extended,
            CliPerfTableStyle::Dots => TableStyle::Dots,
        }
    }
}

impl TableStyle {
    // https://docs.rs/tabled/latest/tabled/settings/style/struct.Style.html
    pub fn stylize(self, table: &mut Table) -> &mut Table {
        match self {
            TableStyle::Empty => table.with(Style::empty()),
            TableStyle::Blank => table.with(Style::blank()),
            TableStyle::Ascii => table.with(Style::ascii()),
            TableStyle::AsciiRounded => table.with(Style::ascii_rounded()),
            TableStyle::Modern => table.with(Style::modern()),
            TableStyle::Sharp => table.with(Style::sharp()),
            TableStyle::Rounded => table.with(Style::rounded()),
            TableStyle::Psql => table.with(Style::psql()),
            TableStyle::Markdown => table.with(Style::markdown()),
            TableStyle::ReStructuredText => table.with(Style::re_structured_text()),
            TableStyle::Extended => table.with(Style::extended()),
            TableStyle::Dots => table.with(Style::dots()),
        }
    }
}
