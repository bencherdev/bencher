use crate::parser::TaskTypes;

use super::{swagger::Swagger, typeshare::Typeshare};

#[derive(Debug)]
pub struct Types {
    pub swagger: Swagger,
    pub typeshare: Typeshare,
}

impl TryFrom<TaskTypes> for Types {
    type Error = anyhow::Error;

    fn try_from(_types: TaskTypes) -> Result<Self, Self::Error> {
        Ok(Self {
            swagger: Swagger {},
            typeshare: Typeshare {},
        })
    }
}

impl Types {
    pub fn exec(&self) -> anyhow::Result<()> {
        self.swagger.exec()?;
        self.typeshare.exec()?;

        Ok(())
    }
}
