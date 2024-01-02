use crate::parser::{TaskTemplate, TaskTemplateKind};

#[derive(Debug)]
pub struct Template {
    #[allow(dead_code)]
    template: TaskTemplateKind,
}

impl TryFrom<TaskTemplate> for Template {
    type Error = anyhow::Error;

    fn try_from(template: TaskTemplate) -> Result<Self, Self::Error> {
        let TaskTemplate { template } = template;
        Ok(Self { template })
    }
}

impl Template {
    #[allow(clippy::unnecessary_wraps)]
    pub fn exec(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
