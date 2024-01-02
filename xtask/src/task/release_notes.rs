use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
};

use camino::Utf8PathBuf;

use crate::parser::TaskReleaseNotes;

#[derive(Debug)]
pub struct ReleaseNotes {
    changelog: Option<Utf8PathBuf>,
    path: Option<Utf8PathBuf>,
}

impl TryFrom<TaskReleaseNotes> for ReleaseNotes {
    type Error = anyhow::Error;

    fn try_from(release_notes: TaskReleaseNotes) -> Result<Self, Self::Error> {
        let TaskReleaseNotes { changelog, path } = release_notes;
        Ok(Self { changelog, path })
    }
}

impl ReleaseNotes {
    pub fn exec(&self) -> anyhow::Result<()> {
        let changelog_path = self
            .changelog
            .clone()
            .unwrap_or_else(|| Utf8PathBuf::from("changelog.md"));
        let changelog_file = File::open(changelog_path)?;

        let file_path = self
            .path
            .clone()
            .unwrap_or_else(|| Utf8PathBuf::from("release-notes.md"));
        let output_file = File::create(file_path)?;

        let buffered_reader = BufReader::new(changelog_file);
        let mut buffered_writer = BufWriter::new(output_file);
        let mut header = true;
        for line in buffered_reader.lines() {
            let line = line?;
            if header {
                if line.starts_with("## Pending") {
                    anyhow::bail!("Release notes still pending");
                } else if line.starts_with("##") {
                    header = false;
                }
                continue;
            }

            if line.starts_with("##") {
                break;
            }
            buffered_writer.write_all(line.as_bytes())?;
            buffered_writer.write_all(&[b'\n'])?;
        }

        Ok(())
    }
}
