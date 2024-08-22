use crate::parser::project::run::CliRunFormat;

#[derive(Debug, Clone, Copy)]
pub enum Format {
    Human,
    Json,
    Html,
}

impl From<CliRunFormat> for Format {
    fn from(fmt: CliRunFormat) -> Self {
        match fmt {
            CliRunFormat::Human => Self::Human,
            CliRunFormat::Json => Self::Json,
            CliRunFormat::Html => Self::Html,
        }
    }
}
