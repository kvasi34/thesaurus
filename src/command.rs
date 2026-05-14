use clap::Parser;

use crate::errors::HandlerError;
use crate::resp2::RespValue;

// Server startup configuration.
#[derive(Debug, Parser)]
#[command(name = "thesaurus")]
pub(crate) struct Cli {
    /// Server IPv4 address
    #[arg(long, default_value = "127.0.0.1", env = "THESAURUS_BIND")]
    pub bind: String,

    /// Server port
    #[arg(long, default_value_t = 6379, env = "THESAURUS_PORT")]
    pub port: u16,

    /// Max TCP socket connections
    #[arg(long, default_value_t = 100, env = "THESAURUS_MAX_CONNECTIONS")]
    pub max_connections: usize,
}

/// Command parsed from raw TCP client input.
#[derive(Debug, PartialEq)]
pub(crate) enum Command {
    /// Ping the server to check if it is alive
    Ping { message: Option<String> },
    /// Get the value for a key
    Get { key: String },
    /// Set a key-value pair
    Set { key: String, value: String },
    /// Delete a key-value pair
    Delete { key: String },
}

impl Command {
    /// Parses a RESP2 line into a `Command` enum. The line is expected to be an array of bulk strings,
    /// where each string is part of the command.
    ///
    /// For example, `PING "hello"` is received as `*2\r\n$4\r\nPING\r\n$5\r\nhello\r\n`.
    pub fn from_resp2(resp_value: &RespValue) -> Result<Self, HandlerError> {
        // Validations
        // Check that the command is of type `RespValue::Array`
        let args = match resp_value {
            RespValue::Array(Some(arr)) => arr,
            _ => {
                return Err(HandlerError::UnexpectedType {
                    expected: "Array",
                    got: resp_value.clone(),
                });
            }
        };

        // Check all command arguments are of type bulk string
        if let Some(bad_arg) = args
            .iter()
            .find(|arg| !matches!(arg, RespValue::BulkString(Some(_))))
        {
            return Err(HandlerError::UnexpectedType {
                expected: "BulkString",
                got: bad_arg.clone(),
            });
        }

        // Use the first argument to determine the command type
        let first_arg = match &args[0] {
            RespValue::BulkString(Some(s)) => s,
            _ => unreachable!(),
        };

        match first_arg.as_str() {
            "PING" => Command::parse_ping_command(args),
            _ => Err(HandlerError::UnknownCommand(first_arg.clone())),
        }
    }

    fn parse_ping_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        if args.len() == 1 {
            return Ok(Command::Ping { message: None });
        }

        let message = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        Ok(Command::Ping {
            message: Some(message),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Package an array of strings into a RespValue::Array of RespValue::BulkString
    fn create_cmd_resp_msg(args: &[&str]) -> RespValue {
        RespValue::Array(Some(
            args.iter()
                .map(|s| RespValue::BulkString(Some(s.to_string())))
                .collect(),
        ))
    }

    #[test]
    fn test_from_resp2_ping() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["PING", "hello"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Ping {
                message: Some("hello".to_string())
            }
        );
    }

    #[test]
    fn test_from_resp2_ping_no_message() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["PING"]));
        assert_eq!(cmd.unwrap(), Command::Ping { message: None });
    }

    #[test]
    fn test_from_resp2_unexpected_type_not_array() {
        let resp_value = RespValue::SimpleString("PING".to_string());
        let cmd = Command::from_resp2(&resp_value);
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::UnexpectedType {
                expected: "Array",
                got: resp_value
            }
        );
    }

    #[test]
    fn test_from_resp2_unexpected_type_not_bulk_string() {
        let inner_resp_value = RespValue::SimpleString("PING".to_string());
        let cmd = Command::from_resp2(&RespValue::Array(Some(vec![inner_resp_value.clone()])));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::UnexpectedType {
                expected: "BulkString",
                got: inner_resp_value
            }
        );
    }

    #[test]
    fn test_from_resp2_unknown_command() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["DOESNOTEXIST", "hello"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::UnknownCommand("DOESNOTEXIST".to_string())
        );
    }
}
