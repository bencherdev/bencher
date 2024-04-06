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
        let client = BencherClient::new(host, token, None, None, None, None);
        Ok(Self { client })
    }
}

impl EmailList {
    pub async fn exec(&self) -> anyhow::Result<()> {
        let users: JsonUsers = self
            .client
            .send_with(|client| async move { client.users_get().per_page(u8::MAX).send().await })
            .await?;
        for user in users.into_inner() {
            println!("{}", user.email);
        }
        Ok(())
    }
}
