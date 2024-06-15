use clap::Parser;

use crate::parser::TaskTemplate;

mod template;

use template::Template;

#[derive(Debug)]
pub struct Task {
    template: Template,
}

impl Task {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            template: TaskTemplate::parse().try_into()?,
        })
    }

    pub fn exec(&self) -> anyhow::Result<()> {
        self.template.exec()
    }
}
