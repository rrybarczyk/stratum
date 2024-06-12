use std::{
    convert::From,
    fmt::Debug,
    sync::{MutexGuard, PoisonError},
};

use roles_logic_sv2::parsers::Mining;

use crate::mempool::error::JdsMempoolError;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(std::fmt::Debug)]
pub enum Error {
    ConfigError(ext_config::ConfigError),
    Io(std::io::Error),
    ChannelSend(Box<dyn std::marker::Send + Debug>),
    ChannelRecv(async_channel::RecvError),
    BinarySv2(binary_sv2::Error),
    Codec(codec_sv2::Error),
    Noise(noise_sv2::Error),
    RolesLogic(roles_logic_sv2::Error),
    Framing(codec_sv2::framing_sv2::Error),
    PoisonLock(String),
    Custom(String),
    Sv2ProtocolError((u32, Mining<'static>)),
    MempoolError(JdsMempoolError),
    ImpossibleToReconstructBlock(String),
    NoLastDeclaredJob,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;
        match self {
            ConfigError(e) => write!(f, "Config error: {:?}", e),
            Io(ref e) => write!(f, "I/O error: `{:?}", e),
            ChannelSend(ref e) => write!(f, "Channel send failed: `{:?}`", e),
            ChannelRecv(ref e) => write!(f, "Channel recv failed: `{:?}`", e),
            BinarySv2(ref e) => write!(f, "Binary SV2 error: `{:?}`", e),
            Codec(ref e) => write!(f, "Codec SV2 error: `{:?}", e),
            Framing(ref e) => write!(f, "Framing SV2 error: `{:?}`", e),
            Noise(ref e) => write!(f, "Noise SV2 error: `{:?}", e),
            RolesLogic(ref e) => write!(f, "Roles Logic SV2 error: `{:?}`", e),
            PoisonLock(ref e) => write!(f, "Poison lock: {:?}", e),
            Custom(ref e) => write!(f, "Custom SV2 error: `{:?}`", e),
            Sv2ProtocolError(ref e) => {
                write!(f, "Received Sv2 Protocol Error from upstream: `{:?}`", e)
            }
            MempoolError(ref e) => write!(f, "Mempool error: `{:?}`", e),
            ImpossibleToReconstructBlock(e) => {
                write!(f, "Error in reconstructing the block: {:?}", e)
            }
            NoLastDeclaredJob => write!(f, "Last declared job not found"),
        }
    }
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

impl From<async_channel::RecvError> for Error {
    fn from(e: async_channel::RecvError) -> Error {
        Error::ChannelRecv(e)
    }
}

impl From<binary_sv2::Error> for Error {
    fn from(e: binary_sv2::Error) -> Error {
        Error::BinarySv2(e)
    }
}

impl From<codec_sv2::Error> for Error {
    fn from(e: codec_sv2::Error) -> Error {
        Error::Codec(e)
    }
}

impl From<noise_sv2::Error> for Error {
    fn from(e: noise_sv2::Error) -> Error {
        Error::Noise(e)
    }
}

impl From<roles_logic_sv2::Error> for Error {
    fn from(e: roles_logic_sv2::Error) -> Error {
        Error::RolesLogic(e)
    }
}

impl<T: 'static + std::marker::Send + Debug> From<async_channel::SendError<T>> for Error {
    fn from(e: async_channel::SendError<T>) -> Error {
        Error::ChannelSend(Box::new(e))
    }
}

impl From<String> for Error {
    fn from(e: String) -> Error {
        Error::Custom(e)
    }
}
impl From<codec_sv2::framing_sv2::Error> for Error {
    fn from(e: codec_sv2::framing_sv2::Error) -> Error {
        Error::Framing(e)
    }
}

impl<T> From<PoisonError<MutexGuard<'_, T>>> for Error {
    fn from(e: PoisonError<MutexGuard<T>>) -> Error {
        Error::PoisonLock(e.to_string())
    }
}

impl From<(u32, Mining<'static>)> for Error {
    fn from(e: (u32, Mining<'static>)) -> Self {
        Error::Sv2ProtocolError(e)
    }
}

impl From<JdsMempoolError> for Error {
    fn from(error: JdsMempoolError) -> Self {
        Error::MempoolError(error)
    }
}
