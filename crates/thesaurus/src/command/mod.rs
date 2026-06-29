mod list;
mod string;

use crate::errors::HandlerError;
use crate::resp2::RespValue;

pub use string::{SetCondition, SetExpiry};

/// Server startup configuration parsed from CLI arguments and environment variables.
#[cfg(feature = "cli")]
#[derive(Debug, clap::Parser)]
#[command(name = "thesaurus")]
pub struct Cli {
    /// Server IPv4 address
    #[arg(long, default_value = "127.0.0.1", env = "THESAURUS_BIND")]
    pub bind: String,

    /// Server port
    #[arg(long, default_value_t = 6379, env = "THESAURUS_PORT")]
    pub port: u16,

    /// INI configuration file path
    #[arg(long, env = "THESAURUS_CONFIG")]
    pub config: Option<String>,
}

/// FlushDb command argument (SYNC or ASYNC).
#[derive(Debug, PartialEq)]
pub enum FlushMode {
    Sync,
    Async,
}

/// Command parsed from raw TCP client input.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// Ping the server to check if it is alive.
    Ping { message: Option<String> },
    /// Get the value for a key.
    Get { key: String },
    /// Set a key-value pair.
    Set {
        key: String,
        value: String,
        condition: Option<SetCondition>,
        expiry: Option<SetExpiry>,
        /// Return the previous value as part of the SET response.
        get: bool,
    },
    /// Delete a key-value pair.
    Delete { keys: Vec<String> },
    /// Get the value for a key and delete key-value pair.
    GetDel { key: String },
    /// Returns if key(s) exists.
    Exists { keys: Vec<String> },
    /// Prepend one or more elements to a list, creating the key if it does not exist.
    LPush { key: String, elements: Vec<String> },
    /// Append one or more elements to a list, creating the key if it does not exist.
    RPush { key: String, elements: Vec<String> },
    /// Prepend one or more elements to a list, only if the key already exists and holds a list.
    LPushX { key: String, elements: Vec<String> },
    /// Append one or more elements to a list, only if the key already exists and holds a list.
    RPushX { key: String, elements: Vec<String> },
    /// Removes and returns the first elements of the list stored at key.
    LPop { key: String, count: Option<u64> },
    /// Removes and returns the last elements of the list stored at key.
    RPop { key: String, count: Option<u64> },
    /// Get the remaining time to live of a key that has a timeout.
    Ttl { key: String },
    /// Returns the absolute Unix timestamp (since January 1, 1970) in seconds at which the given key will expire.
    ExpireTime { key: String },
    /// Returns the absolute Unix timestamp (since January 1, 1970) in milliseconds at which the given key will expire.
    PExpireTime { key: String },
    /// Remove the expiry from a key, making it permanent.
    Persist { key: String },
    /// Set a timeout for a key by specifying the number of seconds representing the TTL (time to live).
    Expire { key: String, seconds: u64 },
    /// Set a timeout for a key by specifying the number of milliseconds representing the TTL (time to live).
    PExpire { key: String, milliseconds: u64 },
    /// Set a timeout for a key at an absolute Unix timestamp in seconds.
    ExpireAt { key: String, deadline_secs: u64 },
    /// Set a timeout for a key at an absolute Unix timestamp in milliseconds.
    PExpireAt { key: String, deadline_ms: u64 },
    /// Get the hash digest for the value stored in the specified key as a hexadecimal string. A hash digest is a fixed-size
    /// numerical representation of a string value, computed using the XXH3 hash algorithm. Can be used for efficient comparison operations.
    Digest { key: String },
    /// No-op command; Thesaurus is a single-store database. Only accepts 0 as a valid index.
    Select { index: u8 },
    /// Returns the number of keys in the database.
    DbSize,
    /// Flush all keys from the database.
    FlushDb { mode: Option<FlushMode> },
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
            "GETDEL" => Command::parse_key_command(args, |key| Command::GetDel { key }),
            "EXISTS" => Command::parse_keys_command(args, |keys| Command::Exists { keys }),
            "LPUSH" => {
                Command::parse_push_command(args, |key, elements| Command::LPush { key, elements })
            }
            "RPUSH" => {
                Command::parse_push_command(args, |key, elements| Command::RPush { key, elements })
            }
            "LPUSHX" => {
                Command::parse_push_command(args, |key, elements| Command::LPushX { key, elements })
            }
            "RPUSHX" => {
                Command::parse_push_command(args, |key, elements| Command::RPushX { key, elements })
            }
            "LPOP" => Command::parse_pop_command(args, |key, count| Command::LPop { key, count }),
            "RPOP" => Command::parse_pop_command(args, |key, count| Command::RPop { key, count }),
            "TTL" => Command::parse_key_command(args, |key| Command::Ttl { key }),
            "EXPIRETIME" => Command::parse_key_command(args, |key| Command::ExpireTime { key }),
            "PEXPIRETIME" => Command::parse_key_command(args, |key| Command::PExpireTime { key }),
            "PERSIST" => Command::parse_key_command(args, |key| Command::Persist { key }),
            "EXPIRE" => Command::parse_expire_commands(args, |key, seconds| Command::Expire {
                key,
                seconds,
            }),
            "PEXPIRE" => Command::parse_expire_commands(args, |key, milliseconds| {
                Command::PExpire { key, milliseconds }
            }),
            "EXPIREAT" => Command::parse_expire_commands(args, |key, deadline_secs| {
                Command::ExpireAt { key, deadline_secs }
            }),
            "PEXPIREAT" => Command::parse_expire_commands(args, |key, deadline_ms| {
                Command::PExpireAt { key, deadline_ms }
            }),
            "DIGEST" => Command::parse_key_command(args, |key| Command::Digest { key }),
            "SELECT" => Command::parse_select_command(args),
            "DBSIZE" => Command::parse_dbsize_command(args),
            "FLUSHDB" => Command::parse_flushdb_command(args),
            _ => Err(HandlerError::UnknownCommand(first_arg.clone())),
        }
    }

    /// Returns `true` if this command mutates store state. Used to check if a command must be appended to the AOF log.
    pub fn is_write(&self) -> bool {
        matches!(
            self,
            Command::Set { .. }
                | Command::Delete { .. }
                | Command::GetDel { .. }
                | Command::LPush { .. }
                | Command::RPush { .. }
                | Command::LPushX { .. }
                | Command::RPushX { .. }
                | Command::LPop { .. }
                | Command::RPop { .. }
                | Command::Persist { .. }
                | Command::Expire { .. }
                | Command::PExpire { .. }
                | Command::ExpireAt { .. }
                | Command::PExpireAt { .. }
                | Command::FlushDb { .. }
        )
    }

    /// Helper function to parse the arguments of a PING command into a `Command::Ping` struct.
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

    /// Helper function to parse the arguments of commands that require a single mandatory `key` argument into the `Command` struct.
    fn parse_key_command(
        args: &[RespValue],
        make_cmd: fn(String) -> Command,
    ) -> Result<Self, HandlerError> {
        check_arity(args, 2)?;

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        Ok(make_cmd(key))
    }

    /// Helper function to parse the arguments of commands that require one or more `keys` arguments into the `Command` struct.
    fn parse_keys_command(
        args: &[RespValue],
        make_cmd: fn(Vec<String>) -> Command,
    ) -> Result<Self, HandlerError> {
        check_min_arity(args, 2)?;

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

    /// Helper function to parse the arguments of a EXPIRE-like commands into relevant struct.
    fn parse_expire_commands(
        args: &[RespValue],
        make_cmd: fn(String, u64) -> Command,
    ) -> Result<Self, HandlerError> {
        check_arity(args, 3)?;

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

        Ok(make_cmd(key, seconds))
    }

    /// Helper function to parse the arguments of a SELECT command into a `Command::Select` struct.
    fn parse_select_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        check_arity(args, 2)?;

        let index = match &args[1] {
            RespValue::BulkString(Some(s)) => s.parse::<u8>().map_err(|_| {
                // Distinguish "not a number" from "a number that overflows u8":
                // if it parses as i64, it is an integer — just out of range for a DB index.
                if s.parse::<i64>().is_ok() {
                    HandlerError::DbIndexOutOfRange
                } else {
                    HandlerError::NotAnInteger(s.clone())
                }
            })?,
            _ => unreachable!(),
        };

        Ok(Command::Select { index })
    }

    /// Helper function to parse the arguments of a DBSIZE command into a `Command::DbSize` struct.
    fn parse_dbsize_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        check_arity(args, 1)?;

        Ok(Command::DbSize)
    }

    /// Helper function to parse the arguments of a FLUSHDB command into a `Command::FlushDb` struct.
    fn parse_flushdb_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        if args.len() > 2 {
            return Err(HandlerError::WrongArity {
                expected: 1,
                got: args.len() as u8,
            });
        }

        let mode = match args.get(1) {
            None => None,
            Some(RespValue::BulkString(Some(s))) => match s.as_str() {
                "SYNC" => Some(FlushMode::Sync),
                "ASYNC" => Some(FlushMode::Async),
                _ => return Err(HandlerError::SyntaxError),
            },
            _ => unreachable!(),
        };

        Ok(Command::FlushDb { mode })
    }
}

/// Helper function to ensure that the correct number of arguments where given.
fn check_arity(args: &[RespValue], expected: usize) -> Result<(), HandlerError> {
    if args.len() != expected {
        return Err(HandlerError::WrongArity {
            expected: expected as u8,
            got: args.len() as u8,
        });
    }

    Ok(())
}

/// Helper function to ensure that at least `min_expected` arguments were given.
pub(super) fn check_min_arity(args: &[RespValue], min_expected: usize) -> Result<(), HandlerError> {
    if args.len() < min_expected {
        return Err(HandlerError::WrongArity {
            expected: min_expected as u8,
            got: args.len() as u8,
        });
    }

    Ok(())
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
                value: "bar".to_string(),
                condition: None,
                expiry: None,
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_nx() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo", "bar", "NX"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: Some(SetCondition::NX),
                expiry: None,
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_xx() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo", "bar", "XX"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: Some(SetCondition::XX),
                expiry: None,
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_ifeq() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&[
            "SET", "foo", "bar", "IFEQ", "oldval",
        ]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: Some(SetCondition::IfEq("oldval".to_string())),
                expiry: None,
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_ifne() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&[
            "SET", "foo", "bar", "IFNE", "oldval",
        ]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: Some(SetCondition::IfNe("oldval".to_string())),
                expiry: None,
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_ifdeq() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&[
            "SET",
            "foo",
            "bar",
            "IFDEQ",
            "000000000000002a",
        ]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: Some(SetCondition::IfDeq(42)),
                expiry: None,
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_ifdne() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&[
            "SET",
            "foo",
            "bar",
            "IFDNE",
            "000000000000002a",
        ]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: Some(SetCondition::IfDne(42)),
                expiry: None,
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_ex() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo", "bar", "EX", "100"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: None,
                expiry: Some(SetExpiry::Ex(100)),
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_px() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo", "bar", "PX", "5000"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: None,
                expiry: Some(SetExpiry::Px(5000)),
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_exat() {
        // 9999999999 secs ≈ year 2286 — safely in the future for the lifetime of this test
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&[
            "SET",
            "foo",
            "bar",
            "EXAT",
            "9999999999",
        ]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: None,
                expiry: Some(SetExpiry::ExAt(9999999999)),
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_pxat() {
        // 9999999999999 ms ≈ year 2286 — safely in the future for the lifetime of this test
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&[
            "SET",
            "foo",
            "bar",
            "PXAT",
            "9999999999999",
        ]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: None,
                expiry: Some(SetExpiry::PxAt(9999999999999)),
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_keepttl() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo", "bar", "KEEPTTL"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: None,
                expiry: Some(SetExpiry::KeepTtl),
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_with_get() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo", "bar", "GET"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: None,
                expiry: None,
                get: true,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_nx_with_ex() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&[
            "SET", "foo", "bar", "NX", "EX", "100",
        ]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: Some(SetCondition::NX),
                expiry: Some(SetExpiry::Ex(100)),
                get: false,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_xx_with_get() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo", "bar", "XX", "GET"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Set {
                key: "foo".to_string(),
                value: "bar".to_string(),
                condition: Some(SetCondition::XX),
                expiry: None,
                get: true,
            }
        );
    }

    #[test]
    fn test_from_resp2_set_ex_missing_value() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo", "bar", "EX"]));
        assert_eq!(cmd.err().unwrap(), HandlerError::SyntaxError);
    }

    #[test]
    fn test_from_resp2_set_ex_not_an_integer() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&[
            "SET",
            "foo",
            "bar",
            "EX",
            "notanumber",
        ]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::NotAnInteger("notanumber".to_string())
        );
    }

    #[test]
    fn test_from_resp2_set_ifdeq_not_an_integer() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&[
            "SET",
            "foo",
            "bar",
            "IFDEQ",
            "notanumber",
        ]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::NotAnInteger("notanumber".to_string())
        );
    }

    #[test]
    fn test_from_resp2_set_unknown_option() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo", "bar", "UNKNOWN"]));
        assert_eq!(cmd.err().unwrap(), HandlerError::SyntaxError);
    }

    #[test]
    fn test_from_resp2_set_duplicate_condition() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SET", "foo", "bar", "NX", "XX"]));
        assert_eq!(cmd.err().unwrap(), HandlerError::SyntaxError);
    }

    #[test]
    fn test_from_resp2_set_duplicate_expiry() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&[
            "SET", "foo", "bar", "EX", "100", "PX", "5000",
        ]));
        assert_eq!(cmd.err().unwrap(), HandlerError::SyntaxError);
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

    #[test]
    fn test_from_resp2_expiretime() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["EXPIRETIME", "foo"]));
        assert_eq!(
            cmd.unwrap(),
            Command::ExpireTime {
                key: "foo".to_string()
            }
        );
    }

    #[test]
    fn test_from_resp2_expiretime_wrong_arity() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["EXPIRETIME", "foo", "bar"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::WrongArity {
                expected: 2,
                got: 3
            }
        );
    }

    #[test]
    fn test_from_resp2_pexpiretime() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["PEXPIRETIME", "foo"]));
        assert_eq!(
            cmd.unwrap(),
            Command::PExpireTime {
                key: "foo".to_string()
            }
        );
    }

    #[test]
    fn test_from_resp2_pexpiretime_wrong_arity() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["PEXPIRETIME", "foo", "bar"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::WrongArity {
                expected: 2,
                got: 3
            }
        );
    }

    #[test]
    fn test_from_resp2_pexpire() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["PEXPIRE", "foo", "5000"]));
        assert_eq!(
            cmd.unwrap(),
            Command::PExpire {
                key: "foo".to_string(),
                milliseconds: 5000,
            }
        );
    }

    #[test]
    fn test_from_resp2_pexpire_wrong_arity() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["PEXPIRE", "foo"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::WrongArity {
                expected: 3,
                got: 2
            }
        );
    }

    #[test]
    fn test_from_resp2_pexpire_not_an_integer() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["PEXPIRE", "foo", "notanumber"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::NotAnInteger("notanumber".to_string())
        );
    }

    #[test]
    fn test_from_resp2_expireat() {
        // 9999999999 secs ≈ year 2286 — safely in the future for the lifetime of this test
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["EXPIREAT", "foo", "9999999999"]));
        assert_eq!(
            cmd.unwrap(),
            Command::ExpireAt {
                key: "foo".to_string(),
                deadline_secs: 9999999999,
            }
        );
    }

    #[test]
    fn test_from_resp2_expireat_wrong_arity() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["EXPIREAT", "foo"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::WrongArity {
                expected: 3,
                got: 2
            }
        );
    }

    #[test]
    fn test_from_resp2_expireat_not_an_integer() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["EXPIREAT", "foo", "notanumber"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::NotAnInteger("notanumber".to_string())
        );
    }

    #[test]
    fn test_from_resp2_pexpireat() {
        // 9999999999999 ms ≈ year 2286 — safely in the future for the lifetime of this test
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["PEXPIREAT", "foo", "9999999999999"]));
        assert_eq!(
            cmd.unwrap(),
            Command::PExpireAt {
                key: "foo".to_string(),
                deadline_ms: 9999999999999,
            }
        );
    }

    #[test]
    fn test_from_resp2_pexpireat_wrong_arity() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["PEXPIREAT", "foo"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::WrongArity {
                expected: 3,
                got: 2
            }
        );
    }

    #[test]
    fn test_from_resp2_pexpireat_not_an_integer() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["PEXPIREAT", "foo", "notanumber"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::NotAnInteger("notanumber".to_string())
        );
    }

    #[test]
    fn test_from_resp2_select() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["SELECT", "0"]));
        assert_eq!(cmd.unwrap(), Command::Select { index: 0 });
    }

    #[test]
    fn test_from_resp2_digest() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["DIGEST", "foo"]));
        assert_eq!(
            cmd.unwrap(),
            Command::Digest {
                key: "foo".to_string()
            }
        )
    }

    #[test]
    fn test_from_resp2_dbsize() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["DBSIZE"]));
        assert_eq!(cmd.unwrap(), Command::DbSize);
    }

    #[test]
    fn test_from_resp2_dbsize_wrong_arity() {
        let cmd = Command::from_resp2(&create_cmd_resp_msg(&["DBSIZE", "foo"]));
        assert_eq!(
            cmd.err().unwrap(),
            HandlerError::WrongArity {
                expected: 1,
                got: 2
            }
        );
    }
}
