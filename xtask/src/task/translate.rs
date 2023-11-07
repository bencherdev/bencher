#![cfg(feature = "plus")]
#![allow(clippy::unwrap_used)]

use std::{fmt, io::Write};

use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client,
};
use camino::Utf8PathBuf;
use chrono::Utc;
use notify_rust::Notification;

use crate::parser::{CliLanguage, CliTranslate};

// https://platform.openai.com/docs/models/gpt-4
const GPT4_MODEL: &str = "gpt-4-0613";

// export OPENAI_API_KEY=sk-xxx
#[derive(Debug)]
pub struct Translate {
    pub input_path: Utf8PathBuf,
    pub lang: Option<Vec<CliLanguage>>,
    pub output_path: Option<Utf8PathBuf>,
}

impl TryFrom<CliTranslate> for Translate {
    type Error = anyhow::Error;

    fn try_from(translate: CliTranslate) -> Result<Self, Self::Error> {
        let CliTranslate {
            input_path,
            lang,
            output_path,
        } = translate;
        Ok(Self {
            input_path,
            lang,
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
                CliLanguage::German => "German",
                CliLanguage::Spanish => "Spanish",
                CliLanguage::French => "French",
                CliLanguage::Japanese => "Japanese",
                CliLanguage::Korean => "Korean",
                CliLanguage::Portuguese => "Brazilian Portuguese",
                CliLanguage::Russian => "Russian",
                CliLanguage::Chinese => "Simplified Chinese",
            }
        )
    }
}

impl CliLanguage {
    // Returns the ISO 639-1 language code for the language
    fn iso_code(self) -> &'static str {
        match self {
            CliLanguage::German => "de",
            CliLanguage::Spanish => "es",
            CliLanguage::French => "fr",
            CliLanguage::Japanese => "ja",
            CliLanguage::Korean => "ko",
            CliLanguage::Portuguese => "pt",
            CliLanguage::Russian => "ru",
            CliLanguage::Chinese => "zh",
        }
    }

    fn all() -> Vec<CliLanguage> {
        vec![
            CliLanguage::German,
            CliLanguage::Spanish,
            CliLanguage::French,
            CliLanguage::Japanese,
            CliLanguage::Korean,
            CliLanguage::Portuguese,
            CliLanguage::Russian,
            CliLanguage::Chinese,
        ]
    }
}

impl Translate {
    #[allow(clippy::unused_async)]
    pub async fn exec(&self) -> anyhow::Result<()> {
        let languages = self.lang.clone().unwrap_or_else(CliLanguage::all);
        let content_path = Utf8PathBuf::from("services/console/src/");
        // services/console/src/ + dir/content/section/en/example.md
        let input_path = content_path.join(&self.input_path);
        let input_file = input_path.file_name().unwrap();
        let input = std::fs::read_to_string(&input_path)?;
        let output_path = self.output_path.clone().unwrap_or_else(||
        // dir/content/section/en/example.md
        input_path
        // dir/content/section/en/
        .parent()
        .unwrap()
        // dir/content/section/
        .parent()
        .unwrap()
        .to_path_buf());

        let start_time = Utc::now();
        for lang in languages.clone() {
            let mut lang_output_path = output_path.clone();
            // content/section/[lang]/
            lang_output_path.push(lang.iso_code());
            std::fs::create_dir_all(&lang_output_path).unwrap();
            // content/section/[lang]/example.md
            lang_output_path.push(input_file);
            println!("From: {input_path}");
            println!("  To: {lang_output_path}");
            println!("Lang: {lang} ({iso})", iso = lang.iso_code());

            let system_input = format!("You are a professional translator for software documentation. Translate the Markdown (MDX with frontmatter metadata) text provided by the user from American English to {lang}. Do NOT translate any of the text inside of Markdown or HTML code blocks nor any URL strings. Keep in mind that the audience for the translation is software developers.");
            let client = Client::new();
            // https://platform.openai.com/docs/models/model-endpoint-compatibility
            let request = CreateChatCompletionRequestArgs::default()
                .model(GPT4_MODEL)
                .messages([
                    ChatCompletionRequestMessageArgs::default()
                        .role(Role::System)
                        .content(&system_input)
                        .build()?,
                    ChatCompletionRequestMessageArgs::default()
                        .role(Role::User)
                        .content(&input)
                        .build()?,
                ])
                .build()?;
            let response = client.chat().create(request).await?;

            println!("\nResponse:\n");
            let mut translation = String::new();
            #[allow(clippy::use_debug)]
            for choice in response.choices {
                println!(
                    "{}: Role: {}  Content: {:?}",
                    choice.index, choice.message.role, choice.message.content
                );
                translation.push_str(&choice.message.content.unwrap_or_default());
            }

            let mut f = std::fs::File::create(&lang_output_path)?;
            f.write_all(translation.as_bytes()).unwrap();
        }
        let end_time = Utc::now();

        let duration = end_time - start_time;
        let languages_str = languages
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let body = format!("Translated in {duration}: {languages_str}",);
        Notification::new()
            .summary("🤖 Translation complete!")
            .body(&body)
            .show()?;

        Ok(())
    }
}
