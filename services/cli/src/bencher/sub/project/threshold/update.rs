use bencher_client::types::{JsonRemoveModel, JsonUpdateModel, JsonUpdateThreshold};
use bencher_json::{ResourceId, ThresholdUuid};

use super::model::Model;
use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::threshold::{CliModel, CliThresholdUpdate, CliUpdateModel},
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub threshold: ThresholdUuid,
    pub model: Option<Model>,
    pub backend: AuthBackend,
}

impl TryFrom<CliThresholdUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliThresholdUpdate) -> Result<Self, Self::Error> {
        let CliThresholdUpdate {
            project,
            threshold,
            model:
                CliUpdateModel {
                    test,
                    min_sample_size,
                    max_sample_size,
                    window,
                    lower_boundary,
                    upper_boundary,
                    remove_model,
                },
            backend,
        } = update;
        let model = if let Some(test) = test {
            let cli_model = CliModel {
                test,
                min_sample_size,
                max_sample_size,
                window,
                lower_boundary,
                upper_boundary,
            };
            Some(cli_model.try_into()?)
        } else if remove_model {
            None
        } else {
            debug_assert!(remove_model, "model or remove_model must be set");
            None
        };
        Ok(Self {
            project,
            threshold,
            model,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateThreshold {
    fn from(update: Update) -> Self {
        let Update { model, .. } = update;
        if let Some(model) = model {
            let Model {
                test,
                min_sample_size,
                max_sample_size,
                window,
                lower_boundary,
                upper_boundary,
            } = model;
            Self {
                subtype_0: Some(JsonUpdateModel {
                    test,
                    min_sample_size,
                    max_sample_size,
                    window,
                    lower_boundary,
                    upper_boundary,
                }),
                subtype_1: None,
            }
        } else {
            Self {
                subtype_0: None,
                subtype_1: Some(JsonRemoveModel { test: () }),
            }
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_threshold_put()
                    .project(self.project.clone())
                    .threshold(self.threshold)
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
