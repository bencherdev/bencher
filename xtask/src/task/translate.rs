use crate::parser::CliTranslate;

const SWAGGER_PATH: &str = "./services/console/src/content/api/swagger.json";

#[derive(Debug)]
pub struct Translate {}

impl TryFrom<CliTranslate> for Translate {
    type Error = anyhow::Error;

    fn try_from(_swagger: CliTranslate) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Translate {
    #[allow(clippy::unused_async)]
    pub async fn exec(&self) -> anyhow::Result<()> {
        let swagger_spec_str = std::fs::read_to_string(SWAGGER_PATH)?;
        // let _prompt = format!("You are a professional translator for software documentation. Translate the {document_kind} text below from {source_language} to {target_language}. Keep in mind that the audience for the translation is software developers.");
        serde_json::from_str(&swagger_spec_str).map_err(Into::into)
    }
}
