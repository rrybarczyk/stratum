use crate::lib::{jdc_config::JdcConfig, Result};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, help = "Path to TOML configuration file")]
    config_path: String,
}

#[allow(clippy::result_large_err)]
pub fn process_cli_args<'a>() -> Result<'a, JdcConfig> {
    let args = Args::parse();
    let config = match config_ext::Config::builder()
        .add_source(config_ext::File::with_name(&args.config_path))
        .build()
    {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("{:?}", e);
            std::process::exit(1)
        }
    };

    let jdc_config: JdcConfig = config.try_deserialize()?;

    Ok(jdc_config)
}
