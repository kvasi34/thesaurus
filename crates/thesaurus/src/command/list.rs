use crate::errors::HandlerError;
use crate::resp2::RespValue;

use super::Command;

impl Command {
    /// Generic method to parse the arguments of the LSET command into relevant enum.
    pub(super) fn parse_lset_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        super::check_arity(args, 4)?;

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        let index = match &args[2] {
            RespValue::BulkString(Some(s)) => s
                .parse::<i64>()
                .map_err(|_| HandlerError::NotAnInteger(s.clone()))?,
            _ => unreachable!(),
        };

        let element = match &args[3] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        Ok(Command::LSet {
            key,
            index,
            element,
        })
    }

    /// Generic method to parse the arguments of the LINDEX command into relevant enum.
    pub(super) fn parse_lindex_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        super::check_arity(args, 3)?;

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        let index = match &args[2] {
            RespValue::BulkString(Some(s)) => s
                .parse::<i64>()
                .map_err(|_| HandlerError::NotAnInteger(s.clone()))?,
            _ => unreachable!(),
        };

        Ok(Command::LIndex { key, index })
    }

    /// Generic method to parse the arguments of the LRANGE command into relevant enum.
    pub(super) fn parse_lrange_command(args: &[RespValue]) -> Result<Self, HandlerError> {
        super::check_arity(args, 4)?;

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        let start = match &args[2] {
            RespValue::BulkString(Some(s)) => s
                .parse::<i64>()
                .map_err(|_| HandlerError::NotAnInteger(s.clone()))?,
            _ => unreachable!(),
        };

        let stop = match &args[3] {
            RespValue::BulkString(Some(s)) => s
                .parse::<i64>()
                .map_err(|_| HandlerError::NotAnInteger(s.clone()))?,
            _ => unreachable!(),
        };

        Ok(Command::LRange { key, start, stop })
    }
}
