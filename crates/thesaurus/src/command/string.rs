use crate::errors::HandlerError;
use crate::resp2::RespValue;

use super::Command;

/// Condition flag for the SET command. The flags in this group are mutually exclusive.
#[derive(Debug, PartialEq)]
pub enum SetCondition {
    /// Only set if the key does not already exist.
    NX,
    /// Only set if the key already exists.
    XX,
    /// Only set if the current value equals the given string.
    IfEq(String),
    /// Only set if the current value does not equal the given string.
    IfNe(String),
    /// Only set if the XXH3 hash digest of the current value equals the given digest.
    IfDeq(u64),
    /// Only set if the XXH3 hash digest of the current value does not equal the given digest.
    IfDne(u64),
}

/// Expiry option for the SET command. The options in this group are mutually exclusive.
#[derive(Debug, PartialEq)]
pub enum SetExpiry {
    /// Set TTL in seconds.
    Ex(u64),
    /// Set TTL in milliseconds.
    Px(u64),
    /// Set expiry as a Unix timestamp in seconds.
    ExAt(u64),
    /// Set expiry as a Unix timestamp in milliseconds.
    PxAt(u64),
    /// Retain the existing TTL.
    KeepTtl,
}

impl Command {
    /// Helper function to parse the arguments of a SET command into a `Command::Set` struct.
    pub(super) fn parse_set_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        if args.len() < 3 {
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

        // Parse arguments
        let mut condition: Option<SetCondition> = None;
        let mut expiry: Option<SetExpiry> = None;
        let mut get = false;

        // Helper closure to safely retrieve the next argument
        let get_next_token = |args: &[RespValue], i: &mut usize| -> Result<String, HandlerError> {
            *i += 1;
            let next_token = args.get(*i);
            match next_token {
                Some(RespValue::BulkString(Some(s))) => Ok(s.clone()),
                _ => Err(HandlerError::SyntaxError),
            }
        };

        // Helper closure to check if the condition has already been defined
        let guard_duplicate_condition =
            |condition: &Option<SetCondition>| -> Result<_, HandlerError> {
                if condition.is_some() {
                    return Err(HandlerError::SyntaxError);
                }

                Ok(())
            };

        let mut i: usize = 3;
        while i < args.len() {
            let token = match &args[i] {
                RespValue::BulkString(Some(s)) => s.as_str(),
                _ => unreachable!(),
            };

            match token {
                "NX" => {
                    guard_duplicate_condition(&condition)?;
                    condition = Some(SetCondition::NX);
                }
                "XX" => {
                    guard_duplicate_condition(&condition)?;
                    condition = Some(SetCondition::XX);
                }
                "IFEQ" => {
                    guard_duplicate_condition(&condition)?;

                    let next_token = get_next_token(args, &mut i)?;
                    condition = Some(SetCondition::IfEq(next_token));
                }
                "IFNE" => {
                    guard_duplicate_condition(&condition)?;

                    let next_token = get_next_token(args, &mut i)?;
                    condition = Some(SetCondition::IfNe(next_token));
                }
                "IFDEQ" | "IFDNE" => {
                    guard_duplicate_condition(&condition)?;

                    let next_token = get_next_token(args, &mut i)?;
                    let n = u64::from_str_radix(&next_token, 16)
                        .map_err(|_| HandlerError::NotAnInteger(next_token))?;
                    condition = Some(match token {
                        "IFDEQ" => SetCondition::IfDeq(n),
                        _ => SetCondition::IfDne(n),
                    });
                }
                "GET" => {
                    get = true;
                }
                "EX" | "PX" | "EXAT" | "PXAT" => {
                    if expiry.is_some() {
                        return Err(HandlerError::SyntaxError);
                    }

                    let next_token = get_next_token(args, &mut i)?;
                    let n = next_token
                        .parse::<u64>()
                        .map_err(|_| HandlerError::NotAnInteger(next_token))?;
                    expiry = Some(match token {
                        "EX" => SetExpiry::Ex(n),
                        "PX" => SetExpiry::Px(n),
                        "EXAT" => SetExpiry::ExAt(n),
                        _ => SetExpiry::PxAt(n),
                    });
                }
                "KEEPTTL" => {
                    if expiry.is_some() {
                        return Err(HandlerError::SyntaxError);
                    }

                    expiry = Some(SetExpiry::KeepTtl);
                }
                _ => {
                    return Err(HandlerError::SyntaxError);
                }
            }

            i += 1;
        }

        Ok(Command::Set {
            key,
            value,
            condition,
            expiry,
            get,
        })
    }
}
