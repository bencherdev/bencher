use crate::parser::TaskTypes;

use super::{spec::Spec, ts::Ts};

#[derive(Debug)]
pub struct Types {
    spec: Spec,
    ts: Ts,
}

impl TryFrom<TaskTypes> for Types {
    type Error = anyhow::Error;

    fn try_from(_task: TaskTypes) -> Result<Self, Self::Error> {
        Ok(Self {
            spec: Spec {},
            ts: Ts {},
        })
    }
}

impl Types {
    #[allow(clippy::unused_self)]
    pub fn exec(&self) -> anyhow::Result<()> {
        self.spec.exec()?;
        self.ts.exec()?;

        Ok(())
    }
}
