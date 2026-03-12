use crate::{
    CliError,
    parser::noise::{CliNoise, CliNoiseFormat},
};

use super::SubCmd;

pub use bencher_noise::NoiseError;

#[derive(Debug, Clone)]
pub struct Noise {
    duration: u64,
    format: bencher_noise::NoiseFormat,
    quiet: bool,
}

impl From<CliNoise> for Noise {
    fn from(cli: CliNoise) -> Self {
        let CliNoise {
            duration,
            format,
            quiet,
        } = cli;
        Self {
            duration,
            format: match format {
                CliNoiseFormat::Human => bencher_noise::NoiseFormat::Human,
                CliNoiseFormat::Json => bencher_noise::NoiseFormat::Json,
            },
            quiet,
        }
    }
}

impl SubCmd for Noise {
    async fn exec(&self) -> Result<(), CliError> {
        self.exec_inner().map_err(Into::into)
    }
}

impl Noise {
    fn exec_inner(&self) -> Result<(), NoiseError> {
        bencher_noise::run_noise(
            self.duration,
            self.format,
            self.quiet,
            &mut std::io::stdout(),
            &mut std::io::stderr(),
        )
    }
}
