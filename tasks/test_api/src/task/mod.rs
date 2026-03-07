use bencher_json::{DEV_BENCHER_API_URL, Jwt, LOCALHOST_BENCHER_API_URL, Url};
use clap::Parser as _;

use crate::{
    parser::{TaskSub, TaskTask},
    task::test::smoke_test::{DEV_ADMIN_BENCHER_API_TOKEN, DEV_BENCHER_API_TOKEN},
};

#[cfg(feature = "plus")]
mod plus;
mod test;

#[cfg(feature = "plus")]
use plus::{oci::Oci, runner::RunnerTest};
use test::{examples::Examples, seed_test::SeedTest, smoke_test::SmokeTest};

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[derive(Debug)]
pub enum Sub {
    SeedTest(SeedTest),
    Examples(Examples),
    SmokeTest(SmokeTest),
    #[cfg(feature = "plus")]
    Oci(Oci),
    #[cfg(feature = "plus")]
    Runner(RunnerTest),
}

impl TryFrom<TaskTask> for Task {
    type Error = anyhow::Error;

    fn try_from(task: TaskTask) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: task.sub.try_into()?,
        })
    }
}

impl TryFrom<TaskSub> for Sub {
    type Error = anyhow::Error;

    fn try_from(sub: TaskSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            TaskSub::Seed(seed_test) => Self::SeedTest(seed_test.try_into()?),
            TaskSub::Examples(examples) => Self::Examples(examples.try_into()?),
            TaskSub::Smoke(smoke_test) => Self::SmokeTest(smoke_test.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Oci(oci) => Self::Oci(oci.try_into()?),
            #[cfg(feature = "plus")]
            TaskSub::Runner(runner) => Self::Runner(runner.try_into()?),
        })
    }
}

impl Task {
    pub fn new() -> anyhow::Result<Self> {
        TaskTask::parse().try_into()
    }

    pub fn exec(&self) -> anyhow::Result<()> {
        self.sub.exec()
    }
}

impl Sub {
    pub fn exec(&self) -> anyhow::Result<()> {
        match self {
            Self::SeedTest(seed_test) => seed_test.exec(),
            Self::Examples(examples) => examples.exec(),
            Self::SmokeTest(smoke_test) => smoke_test.exec(),
            #[cfg(feature = "plus")]
            Self::Oci(oci) => oci.exec(),
            #[cfg(feature = "plus")]
            Self::Runner(runner) => runner.exec(),
        }
    }
}

fn is_dev(url: Option<&Url>) -> bool {
    url.is_some_and(|u| u.as_ref() == DEV_BENCHER_API_URL.as_ref())
}

fn unwrap_url(url: Option<Url>) -> Url {
    url.unwrap_or_else(|| LOCALHOST_BENCHER_API_URL.clone().into())
}

fn unwrap_admin_token(admin_token: Option<Jwt>, is_dev: bool) -> Jwt {
    admin_token.unwrap_or_else(|| {
        if is_dev {
            DEV_ADMIN_BENCHER_API_TOKEN.clone()
        } else {
            Jwt::test_admin_token()
        }
    })
}

fn unwrap_user_token(user_token: Option<Jwt>, is_dev: bool) -> Jwt {
    user_token.unwrap_or_else(|| {
        if is_dev {
            DEV_BENCHER_API_TOKEN.clone()
        } else {
            Jwt::test_token()
        }
    })
}
