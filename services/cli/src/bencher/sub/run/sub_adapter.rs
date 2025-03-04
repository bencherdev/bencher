use crate::parser::run::CliRunCommand;

#[derive(Debug, Clone, Copy)]
pub struct SubAdapter {
    build_time: bool,
    file_size: bool,
}

impl From<&CliRunCommand> for SubAdapter {
    fn from(cmd: &CliRunCommand) -> Self {
        Self {
            build_time: cmd.build_time,
            file_size: cmd
                .file_size
                .as_ref()
                .is_some_and(|paths| !paths.is_empty()),
        }
    }
}

impl From<SubAdapter> for bencher_comment::SubAdapter {
    fn from(sub_adapter: SubAdapter) -> Self {
        let SubAdapter {
            build_time,
            file_size,
        } = sub_adapter;
        Self {
            build_time,
            file_size,
        }
    }
}
