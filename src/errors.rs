use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RespError {
    /// The first byte didn't match any known RESP2 type prefix.
    UnknownPrefix(char),
    /// A length or integer field couldn't be parsed.
    InvalidInteger(String),
    /// A bulk string body contained invalid UTF-8.
    InvalidUtf8,
    /// The connection closed before a complete message was read.
    UnexpectedEof,
}

impl fmt::Display for RespError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RespError::UnknownPrefix(c) => write!(f, "unknown RESP2 prefix: '{}'", c),
            RespError::InvalidInteger(s) => write!(f, "invalid integer: '{}'", s),
            RespError::InvalidUtf8 => write!(f, "bulk string contains invalid UTF-8"),
            RespError::UnexpectedEof => write!(f, "connection closed mid-message"),
        }
    }
}

impl std::error::Error for RespError {}

impl From<RespError> for std::io::Error {
    fn from(e: RespError) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum HandlerError {
    /// The received RESP2 value was not the type required to parse a command.
    UnexpectedType {
        expected: &'static str,
        got: crate::resp2::RespValue,
    },
    /// The received RESP2 value is not a valid command.
    UnknownCommand(String),
    // The received command does not have the required number of arguments.
    WrongArity { expected: u8, got: u8 },
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandlerError::UnexpectedType { expected, got } => {
                write!(
                    f,
                    "unexpected type: expected '{}', got '{:?}'",
                    expected, got
                )
            }
            HandlerError::UnknownCommand(s) => write!(f, "unknown command: '{}'", s),
            HandlerError::WrongArity { expected, got } => write!(f, "wrong number of arguments: expected {}, got {}", expected, got),
        }
    }
}

impl std::error::Error for HandlerError {}

impl From<HandlerError> for std::io::Error {
    fn from(e: HandlerError) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    }
}
