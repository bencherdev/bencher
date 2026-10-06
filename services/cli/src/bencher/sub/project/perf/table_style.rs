use tabled::{Table, settings::Style};

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
            CliPerfTableStyle::Empty => Self::Empty,
            CliPerfTableStyle::Blank => Self::Blank,
            CliPerfTableStyle::Ascii => Self::Ascii,
            CliPerfTableStyle::AsciiRounded => Self::AsciiRounded,
            CliPerfTableStyle::Modern => Self::Modern,
            CliPerfTableStyle::Sharp => Self::Sharp,
            CliPerfTableStyle::Rounded => Self::Rounded,
            CliPerfTableStyle::Psql => Self::Psql,
            CliPerfTableStyle::Markdown => Self::Markdown,
            CliPerfTableStyle::ReStructuredText => Self::ReStructuredText,
            CliPerfTableStyle::Extended => Self::Extended,
            CliPerfTableStyle::Dots => Self::Dots,
        }
    }
}

impl TableStyle {
    // https://docs.rs/tabled/latest/tabled/settings/style/struct.Style.html
    pub fn stylize(self, table: &mut Table) -> &mut Table {
        match self {
            Self::Empty => table.with(Style::empty()),
            Self::Blank => table.with(Style::blank()),
            Self::Ascii => table.with(Style::ascii()),
            Self::AsciiRounded => table.with(Style::ascii_rounded()),
            Self::Modern => table.with(Style::modern()),
            Self::Sharp => table.with(Style::sharp()),
            Self::Rounded => table.with(Style::rounded()),
            Self::Psql => table.with(Style::psql()),
            Self::Markdown => table.with(Style::markdown()),
            Self::ReStructuredText => table.with(Style::re_structured_text()),
            Self::Extended => table.with(Style::extended()),
            Self::Dots => table.with(Style::dots()),
        }
    }
}
