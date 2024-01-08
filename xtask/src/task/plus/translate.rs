use std::{fmt, io::Write};

use async_openai::{
    types::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use camino::Utf8PathBuf;
use chrono::Utc;
use notify_rust::Notification;

use crate::parser::{TaskLanguage, TaskTranslate};

// https://platform.openai.com/docs/models/gpt-4
const GPT4_MODEL: &str = "gpt-4-0613";

// export OPENAI_API_KEY=sk-xxx
#[derive(Debug)]
pub struct Translate {
    pub input_path: Vec<Utf8PathBuf>,
    pub lang: Option<Vec<TaskLanguage>>,
    pub output_path: Option<Utf8PathBuf>,
}

impl TryFrom<TaskTranslate> for Translate {
    type Error = anyhow::Error;

    fn try_from(translate: TaskTranslate) -> Result<Self, Self::Error> {
        let TaskTranslate {
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

impl fmt::Display for TaskLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TaskLanguage::German => "German",
                TaskLanguage::Spanish => "Spanish",
                TaskLanguage::French => "French",
                TaskLanguage::Japanese => "Japanese",
                TaskLanguage::Korean => "Korean",
                TaskLanguage::Portuguese => "Brazilian Portuguese",
                TaskLanguage::Russian => "Russian",
                TaskLanguage::Chinese => "Simplified Chinese",
            }
        )
    }
}

impl TaskLanguage {
    // Returns the ISO 639-1 language code for the language
    fn iso_code(self) -> &'static str {
        match self {
            TaskLanguage::German => "de",
            TaskLanguage::Spanish => "es",
            TaskLanguage::French => "fr",
            TaskLanguage::Japanese => "ja",
            TaskLanguage::Korean => "ko",
            TaskLanguage::Portuguese => "pt",
            TaskLanguage::Russian => "ru",
            TaskLanguage::Chinese => "zh",
        }
    }

    fn all() -> Vec<TaskLanguage> {
        vec![
            TaskLanguage::German,
            TaskLanguage::Spanish,
            TaskLanguage::French,
            TaskLanguage::Japanese,
            TaskLanguage::Korean,
            TaskLanguage::Portuguese,
            TaskLanguage::Russian,
            TaskLanguage::Chinese,
        ]
    }
}

impl Translate {
    #[allow(clippy::unused_async)]
    pub async fn exec(&self) -> anyhow::Result<()> {
        for input_path in &self.input_path {
            let languages = self.lang.clone().unwrap_or_else(TaskLanguage::all);
            let content_path = Utf8PathBuf::from("services/console/src/");
            // services/console/src/ + dir/content/section/en/example.md
            let input_path = if input_path == "scrap" {
                content_path.join("chunks/scrap/en/scrap.mdx")
            } else {
                content_path.join(input_path)
            };
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

                let system_input = format!("You are a professional translator for software documentation. Translate the Markdown (MDX with frontmatter metadata) text provided by the user from American English to {lang}. Do NOT translate any of the text inside of Markdown or HTML code blocks nor any URL strings* (* Only modify internal URL strings that fit the pattern `/docs/*`. Modify these URLs to point to the correct {lang} language version of the page. For example, `/docs/explanation` should be changed to `/docs/{iso}/explanation` for {lang}). Keep in mind that the audience for the translation is software developers.", iso = lang.iso_code());
                let client = Client::new();
                // https://platform.openai.com/docs/models/model-endpoint-compatibility
                let request = CreateChatCompletionRequestArgs::default()
                    .model(GPT4_MODEL)
                    .messages([
                        ChatCompletionRequestSystemMessageArgs::default()
                            .content(system_input.as_str())
                            .build()?
                            .into(),
                        ChatCompletionRequestUserMessageArgs::default()
                            .content(input.as_str())
                            .build()?
                            .into(),
                    ])
                    .build()?;
                let response = client.chat().create(request).await?;

                println!("\nResponse:\n");
                let mut translation = String::new();
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
        }

        Ok(())
    }
}
