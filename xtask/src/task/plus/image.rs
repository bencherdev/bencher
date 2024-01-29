use async_openai::{
    types::{CreateImageRequestArgs, ImageSize, ResponseFormat},
    Client,
};
use chrono::Utc;
use notify_rust::Notification;

use crate::parser::TaskImage;

// https://platform.openai.com/docs/models/dall-e
// const DALL_E_MODEL: &str = "dall-e-3";

// export OPENAI_API_KEY=sk-xxx
#[derive(Debug)]
pub struct Image {
    pub prompt: String,
}

impl TryFrom<TaskImage> for Image {
    type Error = anyhow::Error;

    fn try_from(image: TaskImage) -> Result<Self, Self::Error> {
        let TaskImage { prompt } = image;
        Ok(Self { prompt })
    }
}

impl Image {
    pub async fn exec(&self) -> anyhow::Result<()> {
        let start_time = Utc::now();

        let client = Client::new();
        let request = CreateImageRequestArgs::default()
            .prompt(&self.prompt)
            .response_format(ResponseFormat::Url)
            .size(ImageSize::S1024x1024)
            .user("bencher.dev")
            .build()?;
        let response = client.images().create(request).await?;
        let end_time = Utc::now();

        let paths = response.save("./out").await?;
        for path in paths {
            println!("Image created: {}", path.display());
        }

        let duration = end_time - start_time;
        let body = format!("Written in {duration}");
        Notification::new()
            .summary("🤖 Drawing complete!")
            .body(&body)
            .show()?;

        Ok(())
    }
}
