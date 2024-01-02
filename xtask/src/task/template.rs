use crate::parser::{CliTemplate, CliTemplateKind};

#[derive(Debug)]
pub struct Template {
    template: CliTemplateKind,
}

impl TryFrom<CliTemplate> for Template {
    type Error = anyhow::Error;

    fn try_from(template: CliTemplate) -> Result<Self, Self::Error> {
        let CliTemplate { template } = template;
        Ok(Self { template })
    }
}

impl Template {
    pub fn exec(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
