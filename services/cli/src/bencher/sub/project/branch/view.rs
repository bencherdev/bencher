use bencher_json::{HeadUuid, ResourceId};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::branch::CliBranchView,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub branch: ResourceId,
    pub head: Option<HeadUuid>,
    pub backend: PubBackend,
}

impl TryFrom<CliBranchView> for View {
    type Error = CliError;

    fn try_from(view: CliBranchView) -> Result<Self, Self::Error> {
        let CliBranchView {
            project,
            branch,
            head,
            backend,
        } = view;
        Ok(Self {
            project,
            branch,
            head,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                let mut client = client
                    .proj_branch_get()
                    .project(self.project.clone())
                    .branch(self.branch.clone());

                if let Some(head) = self.head {
                    client = client.head(head);
                }

                client.send().await
            })
            .await?;
        Ok(())
    }
}
