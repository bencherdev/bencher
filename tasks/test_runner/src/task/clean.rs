use crate::parser::TaskClean;

#[derive(Debug)]
pub struct Clean {}

impl TryFrom<TaskClean> for Clean {
    type Error = anyhow::Error;

    fn try_from(_clean: TaskClean) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Clean {
    #[expect(clippy::unused_self)]
    pub fn exec(&self) -> anyhow::Result<()> {
        let work_dir = super::work_dir();
        if work_dir.exists() {
            println!("Cleaning up {work_dir}...");
            std::fs::remove_dir_all(&work_dir)?;
        }
        println!("Done.");
        Ok(())
    }
}
