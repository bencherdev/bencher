use std::fmt;

use bencher_client::types::{
    JsonUpdateBenchmark, JsonUpdateBranch, JsonUpdateMeasure, JsonUpdateTestbed,
};
use bencher_json::{
    BenchmarkName, BenchmarkNameId, BranchName, BranchNameId, JsonBenchmark, JsonBenchmarks,
    JsonBranch, JsonBranches, JsonMeasure, JsonMeasures, JsonTestbed, JsonTestbeds, MeasureNameId,
    NameId, ResourceId, ResourceName, TestbedNameId,
};

use crate::{
    bencher::backend::AuthBackend, cli_println, parser::project::archive::CliArchiveDimension,
};

use super::{ArchiveAction, ArchiveError};

#[derive(Debug, Clone)]
pub enum Dimension {
    Branch(BranchNameId),
    Testbed(TestbedNameId),
    Benchmark(BenchmarkNameId),
    Measure(MeasureNameId),
}

impl From<CliArchiveDimension> for Dimension {
    fn from(dimension: CliArchiveDimension) -> Self {
        #[expect(clippy::panic)]
        if let Some(branch) = dimension.branch {
            Self::Branch(branch)
        } else if let Some(testbed) = dimension.testbed {
            Self::Testbed(testbed)
        } else if let Some(benchmark) = dimension.benchmark {
            Self::Benchmark(benchmark)
        } else if let Some(measure) = dimension.measure {
            Self::Measure(measure)
        } else {
            panic!("No dimension provided")
        }
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Branch(name_id) => write!(f, "branch ({name_id})"),
            Self::Testbed(name_id) => write!(f, "testbed ({name_id})"),
            Self::Benchmark(name_id) => write!(f, "benchmark ({name_id})"),
            Self::Measure(name_id) => write!(f, "measure ({name_id})"),
        }
    }
}

impl Dimension {
    pub async fn archive(
        &self,
        project: &ResourceId,
        action: ArchiveAction,
        backend: &AuthBackend,
    ) -> Result<(), ArchiveError> {
        match self {
            Self::Branch(_) => self.archive_branch(project, action, backend).await,
            Self::Testbed(_) => self.archive_testbed(project, action, backend).await,
            Self::Benchmark(_) => self.archive_benchmark(project, action, backend).await,
            Self::Measure(_) => self.archive_measure(project, action, backend).await,
        }
    }

    pub async fn archive_branch(
        &self,
        project: &ResourceId,
        action: ArchiveAction,
        backend: &AuthBackend,
    ) -> Result<(), ArchiveError> {
        let Self::Branch(name_id) = self.clone() else {
            return Err(ArchiveError::NoDimension {
                dimension: self.clone(),
            });
        };
        let branch: &ResourceId = &match name_id {
            NameId::Uuid(uuid) => uuid.into(),
            NameId::Slug(slug) => slug.into(),
            NameId::Name(name) => match get_branch_by_name(project, &name, action, backend).await {
                Ok(json) => json.uuid.into(),
                Err(err @ ArchiveError::NotFound { .. }) => {
                    return if get_branch_by_name(project, &name, !action, backend)
                        .await
                        .is_ok()
                    {
                        self.log_noop(action);
                        Ok(())
                    } else {
                        Err(err)
                    };
                },
                Err(err) => return Err(err),
            },
        };
        let update = &JsonUpdateBranch {
            name: None,
            slug: None,
            start_point: None,
            archived: Some(action.into()),
        };
        backend
            .send(|client| async move {
                client
                    .proj_branch_patch()
                    .project(project.clone())
                    .branch(branch.clone())
                    .body(update.clone())
                    .send()
                    .await
            })
            .await
            .map_err(|err| ArchiveError::ArchiveDimension {
                dimension: self.clone(),
                err,
            })?;

        self.log_success(action);
        Ok(())
    }

    pub async fn archive_testbed(
        &self,
        project: &ResourceId,
        action: ArchiveAction,
        backend: &AuthBackend,
    ) -> Result<(), ArchiveError> {
        let Self::Testbed(name_id) = self.clone() else {
            return Err(ArchiveError::NoDimension {
                dimension: self.clone(),
            });
        };
        let testbed: &ResourceId = &match name_id {
            NameId::Uuid(uuid) => uuid.into(),
            NameId::Slug(slug) => slug.into(),
            NameId::Name(name) => {
                match get_testbed_by_name(project, &name, action, backend).await {
                    Ok(json) => json.uuid.into(),
                    Err(err @ ArchiveError::NotFound { .. }) => {
                        return if get_testbed_by_name(project, &name, !action, backend)
                            .await
                            .is_ok()
                        {
                            self.log_noop(action);
                            Ok(())
                        } else {
                            Err(err)
                        };
                    },
                    Err(err) => return Err(err),
                }
            },
        };
        let update = &JsonUpdateTestbed {
            name: None,
            slug: None,
            archived: Some(action.into()),
        };
        backend
            .send(|client| async move {
                client
                    .proj_testbed_patch()
                    .project(project.clone())
                    .testbed(testbed.clone())
                    .body(update.clone())
                    .send()
                    .await
            })
            .await
            .map_err(|err| ArchiveError::ArchiveDimension {
                dimension: self.clone(),
                err,
            })?;

        Ok(())
    }

    pub async fn archive_benchmark(
        &self,
        project: &ResourceId,
        action: ArchiveAction,
        backend: &AuthBackend,
    ) -> Result<(), ArchiveError> {
        let Self::Benchmark(name_id) = self.clone() else {
            return Err(ArchiveError::NoDimension {
                dimension: self.clone(),
            });
        };
        let benchmark: &ResourceId = &match name_id {
            NameId::Uuid(uuid) => uuid.into(),
            NameId::Slug(slug) => slug.into(),
            NameId::Name(name) => {
                match get_benchmark_by_name(project, &name, action, backend).await {
                    Ok(json) => json.uuid.into(),
                    Err(err @ ArchiveError::NotFound { .. }) => {
                        return if get_benchmark_by_name(project, &name, !action, backend)
                            .await
                            .is_ok()
                        {
                            self.log_noop(action);
                            Ok(())
                        } else {
                            Err(err)
                        };
                    },
                    Err(err) => return Err(err),
                }
            },
        };
        let update = &JsonUpdateBenchmark {
            name: None,
            slug: None,
            archived: Some(action.into()),
        };
        backend
            .send(|client| async move {
                client
                    .proj_benchmark_patch()
                    .project(project.clone())
                    .benchmark(benchmark.clone())
                    .body(update.clone())
                    .send()
                    .await
            })
            .await
            .map_err(|err| ArchiveError::ArchiveDimension {
                dimension: self.clone(),
                err,
            })?;

        Ok(())
    }

    pub async fn archive_measure(
        &self,
        project: &ResourceId,
        action: ArchiveAction,
        backend: &AuthBackend,
    ) -> Result<(), ArchiveError> {
        let Self::Measure(name_id) = self.clone() else {
            return Err(ArchiveError::NoDimension {
                dimension: self.clone(),
            });
        };
        let measure: &ResourceId = &match name_id {
            NameId::Uuid(uuid) => uuid.into(),
            NameId::Slug(slug) => slug.into(),
            NameId::Name(name) => {
                match get_measure_by_name(project, &name, action, backend).await {
                    Ok(json) => json.uuid.into(),
                    Err(err @ ArchiveError::NotFound { .. }) => {
                        return if get_measure_by_name(project, &name, !action, backend)
                            .await
                            .is_ok()
                        {
                            self.log_noop(action);
                            Ok(())
                        } else {
                            Err(err)
                        };
                    },
                    Err(err) => return Err(err),
                }
            },
        };
        let update = &JsonUpdateMeasure {
            name: None,
            slug: None,
            units: None,
            archived: Some(action.into()),
        };
        backend
            .send(|client| async move {
                client
                    .proj_measure_patch()
                    .project(project.clone())
                    .measure(measure.clone())
                    .body(update.clone())
                    .send()
                    .await
            })
            .await
            .map_err(|err| ArchiveError::ArchiveDimension {
                dimension: self.clone(),
                err,
            })?;

        Ok(())
    }

    fn log_success(&self, action: ArchiveAction) {
        cli_println!("Successfully {} the {self}.", action.as_ref());
    }

    fn log_noop(&self, action: ArchiveAction) {
        cli_println!("The {self} is already {}.", action.as_ref());
    }
}

async fn get_branch_by_name(
    project: &ResourceId,
    name: &BranchName,
    action: ArchiveAction,
    backend: &AuthBackend,
) -> Result<JsonBranch, ArchiveError> {
    let json_list: JsonBranches = backend
        .send_with(|client| async move {
            client
                .proj_branches_get()
                .project(project.clone())
                .name(name.clone())
                .archived(action.is_archived())
                .send()
                .await
        })
        .await
        .map_err(|err| ArchiveError::GetDimension {
            name: name.to_string(),
            err,
        })?;

    one_and_only_one(project, name, json_list)
}

async fn get_testbed_by_name(
    project: &ResourceId,
    name: &ResourceName,
    action: ArchiveAction,
    backend: &AuthBackend,
) -> Result<JsonTestbed, ArchiveError> {
    let json_list: JsonTestbeds = backend
        .send_with(|client| async move {
            client
                .proj_testbeds_get()
                .project(project.clone())
                .name(name.clone())
                .archived(action.is_archived())
                .send()
                .await
        })
        .await
        .map_err(|err| ArchiveError::GetDimension {
            name: name.to_string(),
            err,
        })?;

    one_and_only_one(project, name, json_list)
}

async fn get_benchmark_by_name(
    project: &ResourceId,
    name: &BenchmarkName,
    action: ArchiveAction,
    backend: &AuthBackend,
) -> Result<JsonBenchmark, ArchiveError> {
    let json_list: JsonBenchmarks = backend
        .send_with(|client| async move {
            client
                .proj_benchmarks_get()
                .project(project.clone())
                .name(name.clone())
                .archived(action.is_archived())
                .send()
                .await
        })
        .await
        .map_err(|err| ArchiveError::GetDimension {
            name: name.to_string(),
            err,
        })?;

    one_and_only_one(project, name, json_list)
}

async fn get_measure_by_name(
    project: &ResourceId,
    name: &ResourceName,
    action: ArchiveAction,
    backend: &AuthBackend,
) -> Result<JsonMeasure, ArchiveError> {
    let json_list: JsonMeasures = backend
        .send_with(|client| async move {
            client
                .proj_measures_get()
                .project(project.clone())
                .name(name.clone())
                .archived(action.is_archived())
                .send()
                .await
        })
        .await
        .map_err(|err| ArchiveError::GetDimension {
            name: name.to_string(),
            err,
        })?;

    one_and_only_one(project, name, json_list)
}

fn one_and_only_one<N, L, T>(
    project: &ResourceId,
    name: &N,
    json_list: L,
) -> Result<T, ArchiveError>
where
    N: ToString,
    Vec<T>: From<L>,
{
    let mut vec_json = Vec::from(json_list);
    let count = vec_json.len();
    if let Some(entry) = vec_json.pop() {
        if count == 1 {
            Ok(entry)
        } else {
            Err(ArchiveError::MultipleWithName {
                project: project.to_string(),
                name: name.to_string(),
                count,
            })
        }
    } else {
        Err(ArchiveError::NotFound {
            project: project.to_string(),
            name: name.to_string(),
        })
    }
}
