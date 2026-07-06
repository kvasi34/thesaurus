use std::fmt;

/// Errors that can occur while decoding or encoding RESP2 messages.
#[derive(Debug, Clone, PartialEq)]
pub enum RespError {
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

/// Errors that can occur in a [`crate::handler::Handler`] task.
#[derive(Debug, Clone, PartialEq)]
pub enum HandlerError {
    /// The received RESP2 value was not the type required to parse a command.
    UnexpectedType {
        expected: &'static str,
        got: crate::resp2::RespValue,
    },
    /// The received RESP2 value is not a valid command.
    UnknownCommand(String),
    /// The received command does not have the required number of arguments.
    WrongArity { expected: u8, got: u8 },
    /// The received command does not contain an integer.
    NotAnInteger(String),
    /// The database index provided to SELECT is out of range.
    DbIndexOutOfRange,
    /// The command was called with an unrecognised option or argument.
    SyntaxError,
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedType { expected, got } => {
                write!(
                    f,
                    "unexpected type: expected '{}', got '{:?}'",
                    expected, got
                )
            }
            Self::UnknownCommand(s) => write!(f, "unknown command: '{}'", s),
            Self::WrongArity { expected, got } => write!(
                f,
                "wrong number of arguments: expected {}, got {}",
                expected, got
            ),
            Self::NotAnInteger(s) => write!(f, "not an integer: '{}'", s),
            Self::DbIndexOutOfRange => write!(f, "ERR DB index is out of range"),
            Self::SyntaxError => write!(f, "ERR syntax error"),
        }
    }
}

impl std::error::Error for HandlerError {}

impl From<HandlerError> for std::io::Error {
    fn from(e: HandlerError) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    }
}

/// Errors that can occur in [`crate::store::Store`] operations.
#[derive(Debug, PartialEq)]
pub enum StoreError {
    /// The operation was attempted on a key that holds the wrong data type.
    WrongType,
    /// Out of index error.
    OutOfIndex,
    /// The key does not match any key in the store.
    NoSuchKey,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongType => write!(
                f,
                "WRONGTYPE Operation against a key holding the wrong kind of value"
            ),
            Self::OutOfIndex => write!(f, "ERR index out of range"),
            Self::NoSuchKey => write!(f, "ERR No such key"),
        }
    }
}
