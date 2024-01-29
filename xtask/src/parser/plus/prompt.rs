use camino::Utf8PathBuf;
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct TaskPrompt {
    /// Text prompt
    pub prompt: String,
}

#[derive(Parser, Debug)]
pub struct TaskTranslate {
    /// File input path (relative to `services/console/src/`)
    pub input_path: Vec<Utf8PathBuf>,

    // Target language
    #[clap(value_enum, long)]
    pub lang: Option<Vec<TaskLanguage>>,

    /// File output path
    #[clap(long)]
    pub output_path: Option<Utf8PathBuf>,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum TaskLanguage {
    #[clap(alias = "de")]
    German,
    #[clap(alias = "es")]
    Spanish,
    #[clap(alias = "fr")]
    French,
    #[clap(alias = "ja")]
    Japanese,
    #[clap(alias = "ko")]
    Korean,
    #[clap(alias = "pt")]
    Portuguese,
    #[clap(alias = "ru")]
    Russian,
    #[clap(alias = "zh")]
    Chinese,
}

#[derive(Parser, Debug)]
pub struct TaskImage {
    /// Image prompt
    pub prompt: String,
}
