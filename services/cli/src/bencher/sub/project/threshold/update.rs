use bencher_client::types::{
    JsonRemoveModel, JsonRemoveModelNull, JsonUpdateModel, JsonUpdateModelNull, JsonUpdateThreshold,
};
use bencher_json::{ProjectResourceId, ThresholdUuid};

use super::model::{Model, admit_flat_test, changepoint_model};
use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::threshold::{CliModel, CliThresholdUpdate, CliUpdateModel},
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ProjectResourceId,
    pub threshold: ThresholdUuid,
    pub model: Option<Model>,
    pub changepoint_model: ChangepointUpdate,
    pub backend: AuthBackend,
}

/// What an update does to the changepoint model.
#[derive(Debug, Clone)]
pub enum ChangepointUpdate {
    Keep,
    Remove,
    Set(Model),
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
            changepoint_model: cli_changepoint_model,
            changepoint_remove,
            backend,
        } = update;
        let model = if let Some(test) = test {
            admit_flat_test(test)?;
            let cli_model = CliModel {
                test,
                min_sample_size,
                max_sample_size,
                window,
                lower_boundary,
                upper_boundary,
            };
            Some(cli_model.try_into()?)
        } else {
            debug_assert!(remove_model, "model or remove_model must be set");
            None
        };
        let changepoint_model = if changepoint_remove {
            ChangepointUpdate::Remove
        } else {
            changepoint_model(&cli_changepoint_model)?
                .map_or(ChangepointUpdate::Keep, ChangepointUpdate::Set)
        };
        Ok(Self {
            project,
            threshold,
            model,
            changepoint_model,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateThreshold {
    fn from(update: Update) -> Self {
        let Update {
            model,
            changepoint_model,
            ..
        } = update;
        let none = Self {
            subtype_0: None,
            subtype_1: None,
            subtype_2: None,
            subtype_3: None,
        };
        match (model, changepoint_model) {
            (Some(model), ChangepointUpdate::Keep) => Self {
                subtype_0: Some(model_update(model, None)),
                ..none
            },
            (Some(model), ChangepointUpdate::Set(changepoint)) => Self {
                subtype_0: Some(model_update(model, Some(changepoint.into()))),
                ..none
            },
            (Some(model), ChangepointUpdate::Remove) => Self {
                subtype_2: Some(model_null(model)),
                ..none
            },
            (None, ChangepointUpdate::Keep) => Self {
                subtype_1: Some(JsonRemoveModel {
                    test: (),
                    changepoint_model: None,
                }),
                ..none
            },
            (None, ChangepointUpdate::Set(changepoint)) => Self {
                subtype_1: Some(JsonRemoveModel {
                    test: (),
                    changepoint_model: Some(changepoint.into()),
                }),
                ..none
            },
            (None, ChangepointUpdate::Remove) => Self {
                subtype_3: Some(JsonRemoveModelNull {
                    test: (),
                    changepoint_model: (),
                }),
                ..none
            },
        }
    }
}

fn model_update(
    model: Model,
    changepoint_model: Option<bencher_client::types::Model>,
) -> JsonUpdateModel {
    let Model {
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
        significance,
        confirmations,
    } = model;
    JsonUpdateModel {
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
        significance,
        confirmations,
        changepoint_model,
    }
}

fn model_null(model: Model) -> JsonUpdateModelNull {
    let Model {
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
        significance,
        confirmations,
    } = model;
    JsonUpdateModelNull {
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
        significance,
        confirmations,
        changepoint_model: (),
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

#[cfg(test)]
mod tests {
    use bencher_client::types::JsonUpdateThreshold;
    use clap::Parser as _;
    use serde_json::json;

    use super::Update;
    use crate::parser::project::threshold::CliThresholdUpdate;

    const JWT: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhdXRoIiwiZXhwIjoxNjY5Mjk5NjExLCJpYXQiOjE2NjkyOTc4MTEsImlzcyI6ImJlbmNoZXIuZGV2Iiwic3ViIjoiYUBhLmNvIiwib3JnIjpudWxsfQ.jJmb_nCVJYLD5InaIxsQfS7x87fUsnCYpQK9SrWrKTc";
    const THRESHOLD: &str = "8d2b6c4e-5f3a-4b1c-9e7d-0a1b2c3d4e5f";
    const MODEL: [&str; 4] = ["--test", "t_test", "--upper-boundary", "0.99"];
    const CHANGEPOINT: [&str; 8] = [
        "--changepoint-test",
        "changepoint",
        "--changepoint-max-sample-size",
        "50",
        "--changepoint-upper-boundary",
        "0.05",
        "--changepoint-significance",
        "0.001",
    ];

    // The changepoint flags set the changepoint model, `--changepoint-remove` sends null for it
    // beside the model or its removal, and without either the update leaves it out.
    #[test]
    fn the_changepoint_flags_set_keep_or_remove_the_changepoint_model() {
        let changepoint = json!({
            "test": "changepoint",
            "max_sample_size": 50,
            "upper_boundary": 0.05,
            "significance": 0.001,
        });
        for (flags, expected) in [
            (
                [MODEL.as_slice(), CHANGEPOINT.as_slice()].concat(),
                json!({ "test": "t_test", "upper_boundary": 0.99, "changepoint_model": changepoint }),
            ),
            (
                [["--remove-model"].as_slice(), CHANGEPOINT.as_slice()].concat(),
                json!({ "test": null, "changepoint_model": changepoint }),
            ),
            (
                [MODEL.as_slice(), &["--changepoint-remove"]].concat(),
                json!({ "test": "t_test", "upper_boundary": 0.99, "changepoint_model": null }),
            ),
            (
                vec!["--remove-model", "--changepoint-remove"],
                json!({ "test": null, "changepoint_model": null }),
            ),
            (
                MODEL.to_vec(),
                json!({ "test": "t_test", "upper_boundary": 0.99 }),
            ),
        ] {
            assert_eq!(body(&flags).unwrap(), expected, "{flags:?}");
        }
    }

    // `--changepoint-confirmations` sets the count of confirming scans of the changepoint model.
    #[test]
    fn the_changepoint_confirmations_flag_sets_the_count() {
        let flags = [
            MODEL.as_slice(),
            CHANGEPOINT.as_slice(),
            &["--changepoint-confirmations", "20"],
        ]
        .concat();
        let sent = body(&flags).unwrap();
        assert_eq!(sent["changepoint_model"]["confirmations"], 20, "{sent}");
    }

    // `--changepoint-remove` cannot set a changepoint model too, and the flat model refuses a test
    // that checks a line.
    #[test]
    fn the_changepoint_flags_refuse_a_contradiction() {
        body(
            &[
                MODEL.as_slice(),
                CHANGEPOINT.as_slice(),
                &["--changepoint-remove"],
            ]
            .concat(),
        )
        .unwrap_err();
        let err = body(&[
            "--test",
            "changepoint",
            "--max-sample-size",
            "50",
            "--upper-boundary",
            "0.05",
        ])
        .unwrap_err();
        assert!(
            err.contains("set it with `--changepoint-test`, not `--test`"),
            "{err}"
        );
    }

    fn body(flags: &[&str]) -> Result<serde_json::Value, String> {
        let args = ["bencher", "project", THRESHOLD]
            .into_iter()
            .chain(flags.iter().copied())
            .chain(["--token", JWT]);
        let cli = CliThresholdUpdate::try_parse_from(args).map_err(|err| err.to_string())?;
        let update = Update::try_from(cli).map_err(|err| err.to_string())?;
        serde_json::to_value(JsonUpdateThreshold::from(update)).map_err(|err| err.to_string())
    }
}
