use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq)]
pub enum ErrorKind {
    Incomplete,
    Other(String),
}

impl Display for ErrorKind {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::Incomplete => write!(fmt, "Incomplete data for the block"),
            ErrorKind::Other(msg) => write!(fmt, "Service error: {}", msg),
        }
    }
}

impl ErrorKind {
    pub fn into_err(self) -> Error {
        Error { kind: self }
    }
}

#[derive(Debug, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
}

impl Error {
    pub fn incomplete_error() -> Self {
        ErrorKind::Incomplete.into_err()
    }
    pub fn other_error<M: Into<String>>(msg: M) -> Self {
        ErrorKind::Other(msg.into()).into_err()
    }
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = core::result::Result<T, Error>;
