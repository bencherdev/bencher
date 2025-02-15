use bencher_client::BencherClient;
use bencher_json::JsonUsers;

use crate::parser::TaskEmailList;

#[derive(Debug)]
pub struct EmailList {
    pub client: BencherClient,
}

impl TryFrom<TaskEmailList> for EmailList {
    type Error = anyhow::Error;

    fn try_from(email_list: TaskEmailList) -> Result<Self, Self::Error> {
        let TaskEmailList { host, token } = email_list;
        let mut builder = BencherClient::builder();
        if let Some(host) = host {
            builder = builder.host(host);
        }
        if let Some(token) = token {
            builder = builder.token(token);
        }
        let client = builder.build();
        Ok(Self { client })
    }
}

impl EmailList {
    pub async fn exec(&self) -> anyhow::Result<()> {
        let users: JsonUsers = self
            .client
            .send_with(|client| async move { client.users_get().per_page(u8::MAX).send().await })
            .await?;
        for user in Vec::from(users) {
            if !user.email.as_ref().ends_with("@bencher.dev") {
                println!("{}", user.email);
            }
        }
        Ok(())
    }
}
