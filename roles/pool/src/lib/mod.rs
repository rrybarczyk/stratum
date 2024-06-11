mod error;
pub mod mining_pool;
pub mod pool_config;
pub mod status;
pub mod template_receiver;

pub(crate) use error::{Error, Result};
pub use error::{Error as PoolError, Result as PoolResult};
