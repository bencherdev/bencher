use crate::cli::clap::CliOutput;

#[derive(Debug, Default)]
pub enum Output {
    Headless,
    #[default]
    Web,
    Url(String),
}

impl From<CliOutput> for Output {
    fn from(output: CliOutput) -> Self {
        if output.headless {
            return Self::Headless;
        }
        if output.web {
            return Self::Web;
        }
        if let Some(url) = output.url {
            return Self::Url(url);
        }
        Self::Web
    }
}

impl Output {
    pub fn open(&self, report_str: &str) {
        match &self {
            Self::Headless => println!("{report_str}"),
            Self::Web => println!("{report_str}"),
            Self::Url(url) => println!("{report_str}"),
        }
    }
}
