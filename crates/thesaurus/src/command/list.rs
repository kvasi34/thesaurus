use crate::errors::HandlerError;
use crate::resp2::RespValue;

use super::Command;

impl Command {
    /// Generic method to parse the arguments of PUSH commands into relevant enum.
    pub(super) fn parse_push_command(
        args: &[RespValue],
        make_cmd: fn(String, Vec<String>) -> Command,
    ) -> Result<Self, HandlerError> {
        super::check_min_arity(args, 3)?;

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        let mut elements: Vec<String> = Vec::new();
        for arg in args.iter().skip(2) {
            let key = match arg {
                RespValue::BulkString(Some(s)) => s.clone(),
                _ => unreachable!(),
            };
            elements.push(key);
        }

        Ok(make_cmd(key, elements))
    }

    /// Generic method to parse the arguments of POP commands into relevant enum.
    pub(super) fn parse_pop_command(
        args: &[RespValue],
        make_cmd: fn(String, Option<u64>) -> Command,
    ) -> Result<Self, HandlerError> {
        super::check_min_arity(args, 2)?;

        let key = match &args[1] {
            RespValue::BulkString(Some(s)) => s.clone(),
            _ => unreachable!(),
        };

        let count: Option<u64> = match args.get(2) {
            Some(RespValue::BulkString(Some(s))) => Some(
                s.parse::<u64>()
                    .map_err(|_| HandlerError::NotAnInteger(s.clone()))?,
            ),
            _ => None,
        };

        Ok(make_cmd(key, count))
    }
}
