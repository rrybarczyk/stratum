pub mod config;
mod error;
pub mod mining_pool;
pub mod status;
pub mod template_receiver;

pub(crate) use error::{Error, Result};
pub use error::{Error as PoolError, Result as PoolResult};

pub use config as pool_config;
pub(crate) use config::Config;
pub use config::Config as PoolConfig;
