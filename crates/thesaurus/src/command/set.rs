use crate::errors::HandlerError;
use crate::resp2::RespValue;

use super::Command;

impl Command {
    /// Generic method to parse the arguments of the SMOVE command into relevant enum.
    pub(super) fn parse_smove_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        super::check_arity(args, 4)?;

        let source = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        let destination = match &args[2] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        let member = match &args[3] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        Ok(Command::SMove {
            source,
            destination,
            member,
        })
    }

    /// Generic method to parse the arguments of the SISMEMBER command into relevant enum.
    pub(super) fn parse_sismember_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        super::check_arity(args, 3)?;

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        let member = match &args[2] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        Ok(Command::SIsMember { key, member })
    }

    /// Generic method to parse the arguments of the SMISMEMBER command into relevant enum.
    pub(super) fn parse_smismember_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        super::check_min_arity(args, 3)?;

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        let mut members: Vec<String> = Vec::new();
        for arg in args.iter().skip(2) {
            let key = match arg {
                RespValue::BulkString(Some(s)) => s.clone(),
                _ => unreachable!(),
            };
            members.push(key);
        }

        Ok(Command::SMIsMember { key, members })
    }

    /// Generic method to parse the arguments of the SRANDMEMBER command into relevant enum.
    pub(super) fn parse_srandmember_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        super::check_min_arity(args, 2)?;

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        let count: Option<i64> = match args.get(2) {
            Some(RespValue::BulkString(Some(s))) => Some(
                s.parse::<i64>()
                    .map_err(|_| HandlerError::NotAnInteger(s.clone()))?,
            ),
            _ => None,
        };

        Ok(Command::SRandMember { key, count })
    }
}
