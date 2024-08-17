use crate::parser::project::run::CliRunFormat;

#[derive(Debug, Clone, Copy, Default)]
pub enum Format {
    #[default]
    Text,
    Json,
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
