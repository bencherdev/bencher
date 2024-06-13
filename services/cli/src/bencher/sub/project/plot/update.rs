use bencher_client::types::{JsonPlotPatch, JsonPlotPatchNull, JsonUpdatePlot};
use bencher_json::{Index, PlotUuid, ResourceId, ResourceName, Window};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::plot::CliPlotUpdate,
    CliError,
};

#[derive(Debug, Clone)]
#[allow(clippy::option_option)]
pub struct Update {
    pub project: ResourceId,
    pub plot: PlotUuid,
    pub index: Option<Index>,
    pub title: Option<Option<ResourceName>>,
    pub window: Option<Window>,
    pub backend: AuthBackend,
}

impl TryFrom<CliPlotUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliPlotUpdate) -> Result<Self, Self::Error> {
        let CliPlotUpdate {
            project,
            plot,
            index,
            title,
            window,
            backend,
        } = create;
        Ok(Self {
            project,
            plot,
            index,
            title,
            window,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdatePlot {
    fn from(update: Update) -> Self {
        let Update {
            index,
            title,
            window,
            ..
        } = update;
        match title {
            Some(Some(title)) => Self {
                subtype_0: Some(JsonPlotPatch {
                    index: index.map(Into::into),
                    title: Some(title.into()),
                    window: window.map(Into::into),
                }),
                subtype_1: None,
            },
            Some(None) => Self {
                subtype_0: None,
                subtype_1: Some(JsonPlotPatchNull {
                    index: index.map(Into::into),
                    title: (),
                    window: window.map(Into::into),
                }),
            },
            None => Self {
                subtype_0: Some(JsonPlotPatch {
                    index: index.map(Into::into),
                    title: None,
                    window: window.map(Into::into),
                }),
                subtype_1: None,
            },
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_plot_patch()
                    .project(self.project.clone())
                    .plot(self.plot)
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
