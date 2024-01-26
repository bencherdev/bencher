use crate::parser::TaskIndex;

mod delete;
mod engine;
mod update;

#[derive(Debug)]
pub enum Index {
    Update(update::Update),
    Delete(delete::Delete),
}

impl TryFrom<TaskIndex> for Index {
    type Error = anyhow::Error;

    fn try_from(index: TaskIndex) -> Result<Self, Self::Error> {
        Ok(match index {
            TaskIndex::Update(update) => Self::Update(update.try_into()?),
            TaskIndex::Delete(delete) => Self::Delete(delete.try_into()?),
        })
    }
}

impl Index {
    pub async fn exec(&self) -> anyhow::Result<()> {
        match self {
            Self::Update(update) => update.exec().await,
            Self::Delete(delete) => delete.exec().await,
        }
    }
}
