use std::fs;
#[cfg(unix)]
use std::{fs::Permissions, os::unix::fs::PermissionsExt as _};

use minijinja::Environment;
use serde::Serialize;

use crate::API_VERSION;
use crate::parser::{TaskProduct, TaskTemplate, TaskTemplateKind};

const CLI_TEMPLATES: &str = "services/cli/templates";
const CLI_SH_TEMPLATE: &str = "install-cli.sh.j2";
const CLI_PS1_TEMPLATE: &str = "install-cli.ps1.j2";

const RUNNER_TEMPLATES: &str = "services/runner/templates";
const RUNNER_SH_TEMPLATE: &str = "install-runner.sh.j2";

const ALL_TEMPLATES: &[TemplateKind] = &[
    TemplateKind::CliSh,
    TemplateKind::CliPs1,
    TemplateKind::RunnerSh,
];

#[derive(Debug)]
pub struct Template {
    env: Environment<'static>,
    templates: Vec<TemplateKind>,
}

#[derive(Debug, Clone, Copy)]
pub enum TemplateKind {
    CliSh,
    CliPs1,
    RunnerSh,
}

impl TryFrom<TaskTemplate> for Template {
    type Error = anyhow::Error;

    fn try_from(task: TaskTemplate) -> Result<Self, Self::Error> {
        let TaskTemplate { product, template } = task;

        let templates = match (product, template) {
            // No args: generate everything
            (None, None) => ALL_TEMPLATES.to_vec(),
            // Product only
            (Some(TaskProduct::Cli), None) => vec![TemplateKind::CliSh, TemplateKind::CliPs1],
            (Some(TaskProduct::Runner), None | Some(TaskTemplateKind::Sh)) => {
                vec![TemplateKind::RunnerSh]
            },
            // Product + template kind
            (Some(TaskProduct::Cli), Some(TaskTemplateKind::Sh)) => vec![TemplateKind::CliSh],
            (Some(TaskProduct::Cli), Some(TaskTemplateKind::Ps1)) => vec![TemplateKind::CliPs1],
            (Some(TaskProduct::Runner), Some(TaskTemplateKind::Ps1)) => {
                anyhow::bail!("Runner does not have a PowerShell installer");
            },
            // Template kind without product
            (None, Some(_)) => {
                anyhow::bail!(
                    "Product (cli or runner) is required when specifying a template kind"
                );
            },
        };

        let mut env = Environment::new();
        for &template_kind in &templates {
            let (name, source) = template_kind.read()?;
            env.add_template_owned(name, source)?;
        }

        Ok(Self { env, templates })
    }
}

impl Template {
    #[expect(clippy::use_debug)]
    pub fn exec(&self) -> anyhow::Result<()> {
        for &template_kind in &self.templates {
            let template = self.env.get_template(template_kind.template_file())?;
            let ctx = TemplateContext::new(template_kind);
            let mut rendered = template.render(&ctx)?;
            // minijinja strips trailing newlines from templates
            if !rendered.ends_with('\n') {
                rendered.push('\n');
            }
            let cleaned = newline_converter::dos2unix(&rendered).into_owned();
            let path = format!(
                "{template_dir}/output/{file_name}",
                template_dir = template_kind.template_dir(),
                file_name = template_kind.template_file().trim_end_matches(".j2")
            );
            println!("Using context: {ctx:#?}");
            println!("Saving to: {path}");
            fs::write(&path, cleaned)?;
            #[cfg(unix)]
            fs::set_permissions(&path, Permissions::from_mode(0o755))?;
        }

        Ok(())
    }
}

impl TemplateKind {
    fn template_dir(self) -> &'static str {
        match self {
            Self::CliSh | Self::CliPs1 => CLI_TEMPLATES,
            Self::RunnerSh => RUNNER_TEMPLATES,
        }
    }

    fn template_file(self) -> &'static str {
        match self {
            Self::CliSh => CLI_SH_TEMPLATE,
            Self::CliPs1 => CLI_PS1_TEMPLATE,
            Self::RunnerSh => RUNNER_SH_TEMPLATE,
        }
    }

    fn read(self) -> Result<(&'static str, String), std::io::Error> {
        let file = self.template_file();
        let dir = self.template_dir();
        fs::read_to_string(format!("{dir}/{file}")).map(|t| (file, t))
    }

    fn artifacts(self) -> Vec<TemplateArtifact> {
        match self {
            Self::CliSh => SH_ARTIFACTS.iter().map(Self::as_artifact).collect(),
            Self::CliPs1 => PS1_ARTIFACTS.iter().map(Self::as_artifact).collect(),
            Self::RunnerSh => Vec::new(),
        }
    }

    fn as_artifact(&(target_triple, os_arch): &(&str, &str)) -> TemplateArtifact {
        TemplateArtifact {
            target_triple: target_triple.to_owned(),
            os_arch: os_arch.to_owned(),
            binaries: vec![BENCHER_BIN.to_owned()],
            zip_style: ZipStyle::TempDir,
        }
    }
}

const SH_ARTIFACTS: &[(&str, &str)] = &[
    ("x86_64-unknown-linux-gnu", "linux-x86-64"),
    ("aarch64-unknown-linux-gnu", "linux-arm-64"),
    ("x86_64-apple-darwin", "macos-x86-64"),
    ("aarch64-apple-darwin", "macos-arm-64"),
];
const PS1_ARTIFACTS: &[(&str, &str)] = &[
    ("x86_64-pc-windows-msvc", "windows-x86-64"),
    ("aarch64-pc-windows-msvc", "windows-arm-64"),
];
const BENCHER_BIN: &str = "bencher";

#[derive(Debug, Serialize)]
pub struct TemplateContext {
    /// App version to use
    app_version: &'static str,
    /// Artifacts this installer can fetch
    artifacts: Vec<TemplateArtifact>,
}

impl TemplateContext {
    pub fn new(template_kind: TemplateKind) -> Self {
        let artifacts = template_kind.artifacts();
        Self {
            app_version: API_VERSION,
            artifacts,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TemplateArtifact {
    /// The targets the artifact supports
    target_triple: TargetTriple,
    /// The OS architecture
    os_arch: String,
    /// The binaries the artifact contains (name, assumed at root)
    binaries: Vec<String>,
    /// The style of zip this is
    zip_style: ZipStyle,
}

type TargetTriple = String;

#[expect(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipStyle {
    /// `.zip`
    Zip,
    /// `.tar.<compression>`
    Tar(CompressionImpl),
    /// Don't bundle/compress this, it's just a temp dir
    TempDir,
}

#[expect(dead_code)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CompressionImpl {
    /// `.gz`
    Gzip,
    /// `.xz`
    Xzip,
    /// `.zstd`
    Zstd,
}

impl ZipStyle {
    /// Get the extension used for this kind of zip
    pub fn ext(self) -> &'static str {
        match self {
            ZipStyle::TempDir => "",
            ZipStyle::Zip => ".zip",
            ZipStyle::Tar(compression) => match compression {
                CompressionImpl::Gzip => ".tar.gz",
                CompressionImpl::Xzip => ".tar.xz",
                CompressionImpl::Zstd => ".tar.zstd",
            },
        }
    }
}

impl Serialize for ZipStyle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.ext())
    }
}
