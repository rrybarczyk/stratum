use core::fmt;

#[repr(C)]
#[derive(Debug)]
pub enum Error {
    /// Error if Noise protocol state is not as expected
    UnexpectedNoiseState,
    /// Errors if there are missing bytes in the Noise protocol
    MissingBytes(usize),
    /// Catch all
    Todo,
}

pub type Result<T> = core::result::Result<T, Error>;

impl From<()> for Error {
    fn from(_: ()) -> Self {
        Error::Todo
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Error::*;
        match self {
            UnexpectedNoiseState => {
                write!(f, "Noise state is incorrect")
            }
            MissingBytes(u) => write!(f, "Missing `{}` Noise bytes", u),
            Todo => write!(f, "Codec Sv2 Error: TODO"),
        }
    }
}
