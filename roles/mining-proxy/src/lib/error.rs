use async_channel::SendError;
use codec_sv2::StandardEitherFrame;
use roles_logic_sv2::parsers::PoolMessages;
use std::net::SocketAddr;

pub type Message = PoolMessages<'static>;
pub type EitherFrame = StandardEitherFrame<Message>;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    ConfigError(ext_config::ConfigError),
    Io(std::io::Error),
    SendError(SendError<EitherFrame>),
    UpstreamNotAvailabe(SocketAddr),
    SetupConnectionError(String),
}

impl From<ext_config::ConfigError> for Error {
    fn from(e: ext_config::ConfigError) -> Error {
        Error::ConfigError(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e)
    }
}

impl From<SendError<EitherFrame>> for Error {
    fn from(error: SendError<EitherFrame>) -> Self {
        Error::SendError(error)
    }
}
