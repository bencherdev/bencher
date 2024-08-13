use crate::parser::project::run::CliRunFormat;

#[derive(Debug, Clone, Copy, Default)]
pub enum Format {
    /// Text (default)
    #[default]
    Text,
    /// JSON
    Json,
    /// HTML
    Html,
}

impl From<CliRunFormat> for Format {
    fn from(fmt: CliRunFormat) -> Self {
        match fmt {
            CliRunFormat::Text => Self::Text,
            CliRunFormat::Json => Self::Json,
            CliRunFormat::Html => Self::Html,
        }
    }
}
