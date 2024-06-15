#[cfg(unix)]
use std::{fs::Permissions, os::unix::fs::PermissionsExt};

use minijinja::Environment;
use serde::Serialize;

use crate::parser::{TaskTemplate, TaskTemplateKind};
use crate::API_VERSION;

const CLI_TEMPLATES: &str = "services/cli/templates";
const SH_TEMPLATE: &str = "install-cli.sh.j2";
const PS1_TEMPLATE: &str = "install-cli.ps1.j2";
const TEMPLATES: &[TemplateKind] = &[TemplateKind::Sh, TemplateKind::Ps1];

#[derive(Debug)]
pub struct Template {
    env: Environment<'static>,
    templates: Vec<TemplateKind>,
}

#[derive(Debug, Clone, Copy)]
pub enum TemplateKind {
    Sh,
    Ps1,
}

impl TryFrom<TaskTemplate> for Template {
    type Error = anyhow::Error;

    fn try_from(template: TaskTemplate) -> Result<Self, Self::Error> {
        let TaskTemplate { template } = template;

        let mut env = Environment::new();
        env.set_loader(minijinja::path_loader(CLI_TEMPLATES));

        let templates = if let Some(template) = template {
            let t = TemplateKind::from(template);
            let (name, source) = t.read()?;
            env.add_template_owned(name, source)?;
            vec![t]
        } else {
            let mut templates = Vec::with_capacity(TEMPLATES.len());
            for &template_kind in TEMPLATES {
                let (name, source) = template_kind.read()?;
                env.add_template_owned(name, source)?;
                templates.push(template_kind);
            }
            templates
        };

        Ok(Self { env, templates })
    }
}

impl Template {
    #[allow(clippy::use_debug)]
    pub fn exec(&self) -> anyhow::Result<()> {
        for &template_kind in &self.templates {
            let template = self.env.get_template(template_kind.as_ref())?;
            let ctx = TemplateContext::new(template_kind);
            let mut rendered = template.render(&ctx)?;
            // minijinja strips trailing newlines from templates
            if !rendered.ends_with('\n') {
                rendered.push('\n');
            }
            let cleaned = newline_converter::dos2unix(&rendered).into_owned();
            let path = format!(
                "{CLI_TEMPLATES}/output/{file_name}",
                file_name = template_kind.as_ref().trim_end_matches(".j2")
            );
            println!("Using context: {ctx:#?}");
            println!("Saving to: {path}");
            std::fs::write(&path, cleaned)?;
            #[cfg(unix)]
            std::fs::set_permissions(&path, Permissions::from_mode(0o755))?;
        }

        Ok(())
    }
}

impl From<TaskTemplateKind> for TemplateKind {
    fn from(template: TaskTemplateKind) -> Self {
        match template {
            TaskTemplateKind::Sh => Self::Sh,
            TaskTemplateKind::Ps1 => Self::Ps1,
        }
    }
}

impl AsRef<str> for TemplateKind {
    fn as_ref(&self) -> &str {
        match self {
            Self::Sh => SH_TEMPLATE,
            Self::Ps1 => PS1_TEMPLATE,
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
    ("x86_64-pc-windows-msvc", "windows-x86-64.exe"),
    ("aarch64-pc-windows-msvc", "windows-arm-64.exe"),
];
fn artifact_name(os_arch: &str) -> String {
    format!("bencher-v{API_VERSION}-{os_arch}")
}
const BENCHER_BIN: &str = "bencher";

impl TemplateKind {
    pub fn read(self) -> Result<(&'static str, String), std::io::Error> {
        let file = match self {
            Self::Sh => SH_TEMPLATE,
            Self::Ps1 => PS1_TEMPLATE,
        };
        std::fs::read_to_string(format!("{CLI_TEMPLATES}/{file}")).map(|t| (file, t))
    }

    pub fn artifacts(self) -> Vec<TemplateArtifact> {
        match self {
            Self::Sh => SH_ARTIFACTS.iter().map(Self::as_artifact).collect(),
            Self::Ps1 => PS1_ARTIFACTS.iter().map(Self::as_artifact).collect(),
        }
    }

    fn as_artifact(&(target_triple, os_arch): &(&str, &str)) -> TemplateArtifact {
        TemplateArtifact {
            name: artifact_name(os_arch),
            target_triple: (target_triple).to_owned(),
            binaries: vec![BENCHER_BIN.to_owned()],
            zip_style: ZipStyle::TempDir,
        }
    }
}

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
    /// The name of the artifact
    name: String,
    /// The targets the artifact supports
    target_triple: TargetTriple,
    /// The binaries the artifact contains (name, assumed at root)
    binaries: Vec<String>,
    /// The style of zip this is
    zip_style: ZipStyle,
}

type TargetTriple = String;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipStyle {
    /// `.zip`
    Zip,
    /// `.tar.<compression>`
    Tar(CompressionImpl),
    /// Don't bundle/compress this, it's just a temp dir
    TempDir,
}

#[allow(dead_code)]
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
