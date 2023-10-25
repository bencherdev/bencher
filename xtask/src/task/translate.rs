use std::fmt;

use camino::Utf8PathBuf;

use crate::parser::{CliLanguage, CliTranslate};

const SWAGGER_PATH: &str = "./services/console/src/content/api/swagger.json";

#[derive(Debug)]
pub struct Translate {
    pub lang: CliLanguage,
    pub input_path: Utf8PathBuf,
    pub output_path: Option<Utf8PathBuf>,
}

impl TryFrom<CliTranslate> for Translate {
    type Error = anyhow::Error;

    fn try_from(translate: CliTranslate) -> Result<Self, Self::Error> {
        let CliTranslate {
            lang,
            input_path,
            output_path,
        } = translate;
        Ok(Self {
            lang,
            input_path,
            output_path,
        })
    }
}

impl fmt::Display for CliLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                CliLanguage::Arabic => "Modern Standard Arabic",
                CliLanguage::Chinese => "Simplified Chinese",
                CliLanguage::Spanish => "Spanish",
                CliLanguage::French => "French",
                CliLanguage::German => "German",
                CliLanguage::Japanese => "Japanese",
                CliLanguage::Kannada => "Kannada",
                CliLanguage::Portuguese => "Portuguese",
                CliLanguage::Russian => "Russian",
            }
        )
    }
}

impl Translate {
    #[allow(clippy::unused_async)]
    pub async fn exec(&self) -> anyhow::Result<()> {
        let swagger_spec_str = std::fs::read_to_string(SWAGGER_PATH)?;
        // let _prompt = format!("You are a professional translator for software documentation. Translate the Markdown (MDX) text below from American English to {target_language}. Keep in mind that the audience for the translation is software developers.");
        serde_json::from_str(&swagger_spec_str).map_err(Into::into)
    }
}
