use clap::Parser;

use crate::parser::TaskReleaseNotes;

mod release_notes;

use release_notes::ReleaseNotes;

#[derive(Debug)]
pub struct Task {
    release_notes: ReleaseNotes,
}

impl Task {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            release_notes: TaskReleaseNotes::parse().try_into()?,
        })
    }

    pub fn exec(&self) -> anyhow::Result<()> {
        self.release_notes.exec()
    }
}
