use std::io::Write;

use async_openai::{
    types::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use chrono::Utc;
use notify_rust::Notification;

use crate::parser::TaskPrompt;

// https://platform.openai.com/docs/models/gpt-4
const GPT4_MODEL: &str = "gpt-4-0613";

// export OPENAI_API_KEY=sk-xxx
#[derive(Debug)]
pub struct Prompt {
    pub prompt: String,
}

impl TryFrom<TaskPrompt> for Prompt {
    type Error = anyhow::Error;

    fn try_from(translate: TaskPrompt) -> Result<Self, Self::Error> {
        let TaskPrompt { prompt } = translate;
        Ok(Self { prompt })
    }
}

impl Prompt {
    #[allow(clippy::unused_async)]
    pub async fn exec(&self) -> anyhow::Result<()> {
        let start_time = Utc::now();
        let system_input = "You are a professional technical writer for software documentation. Write your documentation in Markdown using American English.";
        let client = Client::new();
        // https://platform.openai.com/docs/models/model-endpoint-compatibility
        let request = CreateChatCompletionRequestArgs::default()
            .model(GPT4_MODEL)
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(system_input)
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content(self.prompt.as_str())
                    .build()?
                    .into(),
            ])
            .build()?;
        let response = client.chat().create(request).await?;
        let end_time = Utc::now();

        println!("\nResponse:\n");
        let mut resp = String::new();
        for choice in response.choices {
            println!(
                "{}: Role: {}  Content: {:?}",
                choice.index, choice.message.role, choice.message.content
            );
            resp.push_str(&choice.message.content.unwrap_or_default());
        }

        let output_path = format!("out-prompt-{start_time}.txt");
        let mut f = std::fs::File::create(output_path)?;
        f.write_all(resp.as_bytes())?;

        let duration = end_time - start_time;
        let body = format!("Written in {duration}",);
        Notification::new()
            .summary("🤖 Writing complete!")
            .body(&body)
            .show()?;

        Ok(())
    }
}
