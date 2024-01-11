use crate::parser::TaskLicense;

mod generate;
mod validate;

#[derive(Debug)]
pub enum License {
    Generate(generate::Generate),
    Validate(validate::Validate),
}

impl TryFrom<TaskLicense> for License {
    type Error = anyhow::Error;

    fn try_from(project: TaskLicense) -> Result<Self, Self::Error> {
        Ok(match project {
            TaskLicense::Generate(generate) => Self::Generate(generate.try_into()?),
            TaskLicense::Validate(validate) => Self::Validate(validate.try_into()?),
        })
    }
}

impl License {
    pub fn exec(&self) -> anyhow::Result<()> {
        match self {
            Self::Generate(generate) => generate.exec(),
            Self::Validate(validate) => validate.exec(),
        }
    }
}
