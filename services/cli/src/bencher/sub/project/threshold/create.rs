use bencher_client::types::JsonNewThreshold;
use bencher_json::{BranchNameId, MeasureNameId, ProjectResourceId, TestbedNameId};

use super::model::{Model, admit_flat_test, changepoint_model};
use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::threshold::CliThresholdCreate,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ProjectResourceId,
    pub branch: BranchNameId,
    pub testbed: TestbedNameId,
    pub measure: MeasureNameId,
    pub model: Model,
    pub changepoint_model: Option<Model>,
    pub backend: AuthBackend,
}

impl TryFrom<CliThresholdCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliThresholdCreate) -> Result<Self, Self::Error> {
        let CliThresholdCreate {
            project,
            branch,
            testbed,
            measure,
            model,
            changepoint_model: cli_changepoint_model,
            backend,
        } = create;
        admit_flat_test(model.test)?;
        Ok(Self {
            project,
            branch,
            testbed,
            measure,
            model: model.try_into()?,
            changepoint_model: changepoint_model(&cli_changepoint_model)?,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewThreshold {
    fn from(create: Create) -> Self {
        let Create {
            branch,
            testbed,
            model,
            changepoint_model,
            measure,
            ..
        } = create;
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
        Self {
            branch: branch.into(),
            testbed: testbed.into(),
            // The CLI creates the bare threshold: the conventional `value` name of
            // every variant. A threshold that checks a name or only some variants
            // is created through the API.
            parameters: None,
            measure: measure.into(),
            metric: None,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            significance,
            confirmations,
            changepoint_model: changepoint_model.map(Into::into),
        }
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_threshold_post()
                    .project(self.project.clone())
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
    use bencher_client::types::JsonNewThreshold;
    use clap::Parser as _;
    use serde_json::json;

    use super::Create;
    use crate::parser::project::threshold::CliThresholdCreate;

    const JWT: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhdXRoIiwiZXhwIjoxNjY5Mjk5NjExLCJpYXQiOjE2NjkyOTc4MTEsImlzcyI6ImJlbmNoZXIuZGV2Iiwic3ViIjoiYUBhLmNvIiwib3JnIjpudWxsfQ.jJmb_nCVJYLD5InaIxsQfS7x87fUsnCYpQK9SrWrKTc";
    const THRESHOLD: [&str; 8] = [
        "project",
        "--branch",
        "main",
        "--testbed",
        "localhost",
        "--measure",
        "latency",
        "--test",
    ];
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

    // The changepoint flags fill the nested changepoint model beside the flat model.
    #[test]
    fn the_changepoint_flags_fill_the_changepoint_model() {
        let sent = body(
            &[
                &["t_test", "--upper-boundary", "0.99"],
                CHANGEPOINT.as_slice(),
            ]
            .concat(),
        )
        .unwrap();
        assert_eq!(
            sent,
            json!({
                "branch": "main",
                "testbed": "localhost",
                "measure": "latency",
                "test": "t_test",
                "upper_boundary": 0.99,
                "changepoint_model": {
                    "test": "changepoint",
                    "max_sample_size": 50,
                    "upper_boundary": 0.05,
                    "significance": 0.001,
                },
            })
        );
        let sent = body(&["t_test", "--upper-boundary", "0.99"]).unwrap();
        assert_eq!(sent.get("changepoint_model"), None);
    }

    // `--changepoint-confirmations` sets the count of confirming scans, and one out of its range is
    // refused as the flag is read.
    #[test]
    fn the_changepoint_confirmations_flag_sets_the_count() {
        let flags = |confirmations: &'static str| {
            [
                &["t_test", "--upper-boundary", "0.99"],
                CHANGEPOINT.as_slice(),
                &["--changepoint-confirmations", confirmations],
            ]
            .concat()
        };
        let sent = body(&flags("3")).unwrap();
        assert_eq!(sent["changepoint_model"]["confirmations"], 3, "{sent}");
        for out_of_range in ["0", "21"] {
            let err = body(&flags(out_of_range)).unwrap_err();
            assert!(err.contains("`confirmations`"), "{out_of_range}: {err}");
        }
    }

    // A changepoint flag needs the changepoint test, and the changepoint model is validated before
    // it is sent.
    #[test]
    fn the_changepoint_model_needs_its_test_and_its_fields() {
        body(&[
            "t_test",
            "--upper-boundary",
            "0.99",
            "--changepoint-significance",
            "0.001",
        ])
        .unwrap_err();
        let without_window = [&CHANGEPOINT[..2], &CHANGEPOINT[4..]].concat();
        let err = body(
            &[
                &["t_test", "--upper-boundary", "0.99"],
                without_window.as_slice(),
            ]
            .concat(),
        )
        .unwrap_err();
        assert!(err.contains("`max_sample_size`"), "{err}");
    }

    // The flat model refuses a test that checks a line, naming the flag that sets one.
    #[test]
    fn the_model_refuses_a_line_test() {
        let err = body(&[
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

    /// The request body the flags send after the flat `--test`.
    fn body(flags: &[&str]) -> Result<serde_json::Value, String> {
        let args = std::iter::once("bencher")
            .chain(THRESHOLD)
            .chain(flags.iter().copied())
            .chain(["--token", JWT]);
        let cli = CliThresholdCreate::try_parse_from(args).map_err(|err| err.to_string())?;
        let create = Create::try_from(cli).map_err(|err| err.to_string())?;
        serde_json::to_value(JsonNewThreshold::from(create)).map_err(|err| err.to_string())
    }
}
