use crate::lib::{Config, Result};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, help = "Path to TOML configuration file")]
    config_path: String,
}

#[allow(clippy::result_large_err)]
pub fn process_cli_args() -> Result<Config> {
    let args = Args::parse();
    let config = ext_config::Config::builder()
        .add_source(ext_config::File::with_name(&args.config_path))
        .build()
        .unwrap_or_else(|e| {
            tracing::error!("{}", e);
            std::process::exit(1);
        });

    let pool_config = config.try_deserialize::<Config>()?;

    Ok(pool_config)
}
