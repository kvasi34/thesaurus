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
    Delete { keys: Vec<String> },
    /// Returns if key(s) exists.
    Exists { keys: Vec<String> },
    /// Get the remaining time to live of a key that has a timeout
    Ttl { key: String },
    /// Remove the expiry from a key, making it permanent
    Persist { key: String },
    /// Set a timeout  key
    Expire { key: String, seconds: u64 },
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

        // Call the appropriate parser function for each command type
        match first_arg.as_str() {
            "PING" => Command::parse_ping_command(args),
            "GET" => Command::parse_key_command(args, |key| Command::Get { key }),
            "SET" => Command::parse_set_command(args),
            "DEL" => Command::parse_keys_command(args, |keys| Command::Delete { keys }),
            "EXISTS" => Command::parse_keys_command(args, |keys| Command::Exists { keys }),
            "TTL" => Command::parse_key_command(args, |key| Command::Ttl { key }),
            "PERSIST" => Command::parse_key_command(args, |key| Command::Persist { key }),
            "EXPIRE" => Command::parse_expire_command(args),
            _ => Err(HandlerError::UnknownCommand(first_arg.clone())),
        }
    }

    // Helper function to parse the arguments of a PING command into a `Command::Ping` struct
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

    // Helper function to parse the arguments of commands that require a single mandatory `key` argument into the `Command` struct
    fn parse_key_command(
        args: &[RespValue],
        make_cmd: fn(String) -> Command,
    ) -> Result<Self, HandlerError> {
        if args.len() != 2 {
            return Err(HandlerError::WrongArity {
                expected: 2,
                got: args.len() as u8,
            });
        }

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        Ok(make_cmd(key))
    }

    // Helper function to parse the arguments of a SET command into a `Command::Set` struct
    fn parse_set_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        if args.len() != 3 {
            return Err(HandlerError::WrongArity {
                expected: 3,
                got: args.len() as u8,
            });
        }

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };
        let value = match &args[2] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        Ok(Command::Set { key, value })
    }

    // Helper function to parse the arguments of commands that require one or more `keys` arguments into the `Command` struct
    fn parse_keys_command(
        args: &[RespValue],
        make_cmd: fn(Vec<String>) -> Command,
    ) -> Result<Self, HandlerError> {
        if args.len() < 2 {
            return Err(HandlerError::WrongArity {
                expected: 2,
                got: args.len() as u8,
            });
        }

        let mut keys: Vec<String> = Vec::new();
        for arg in args.iter().skip(1) {
            let key = match arg {
                RespValue::BulkString(Some(s)) => s.clone(),
                _ => unreachable!(),
            };
            keys.push(key);
        }

        Ok(make_cmd(keys))
    }

    // Helper function to parse the arguments of an EXPIRE command into a `Command::Expire` struct
    fn parse_expire_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        if args.len() != 3 {
            return Err(HandlerError::WrongArity {
                expected: 3,
                got: args.len() as u8,
            });
        }

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };
        let seconds = match &args[2] {
            RespValue::BulkString(Some(s)) => s
                .parse::<u64>()
                .map_err(|_| HandlerError::NotAnInteger(s.clone()))?,
            _ => unreachable!(),
        };

        Ok(Command::Expire { key, seconds })
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
    fn test_from_resp2_get() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["GET", "foo"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Get {
                key: "foo".to_string()
            }
        );
    }

    #[test]
    fn test_from_resp2_get_wrong_arity() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["GET", "foo", "bar"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::WrongArity {
                expected: 2,
                got: 3
            }
        );
    }

    #[test]
    fn test_from_resp2_set() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo", "bar"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string()
            }
        );
    }

    #[test]
    fn test_from_resp2_set_wrong_arity() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::WrongArity {
                expected: 3,
                got: 2
            }
        );
    }

    #[test]
    fn test_from_resp2_del() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["DEL", "foo"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Delete {
                keys: vec!["foo".to_string()]
            }
        );
    }

    #[test]
    fn test_from_resp2_del_multiple_keys() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["DEL", "foo", "bar", "baz"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Delete {
                keys: vec!["foo".to_string(), "bar".to_string(), "baz".to_string()]
            }
        );
    }

    #[test]
    fn test_from_resp2_del_wrong_arity() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["DEL"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::WrongArity {
                expected: 2,
                got: 1
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
