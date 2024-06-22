pub mod config;
pub(crate) mod error;
pub mod job_declarator;
pub mod mempool;
pub mod status;

pub(crate) use config::Config;
pub use config::Config as JdsConfig;

pub(crate) use error::{Error, Result};
pub use error::{Error as JdsError, Result as JdsResult};

use codec_sv2::{StandardEitherFrame, StandardSv2Frame};
use roles_logic_sv2::parsers::PoolMessages as JdsMessages;

pub type Message = JdsMessages<'static>;
pub type StdFrame = StandardSv2Frame<Message>;
pub type EitherFrame = StandardEitherFrame<Message>;
