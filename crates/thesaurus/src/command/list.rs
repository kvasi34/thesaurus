use crate::errors::HandlerError;
use crate::resp2::RespValue;

use super::Command;

impl Command {
    /// Generic method to parse the arguments of a PUSH-like commands into relevant enum.
    pub(super) fn parse_push_command(
        args: &[RespValue],
        make_cmd: fn(String, Vec<String>) -> Command,
    ) -> Result<Self, HandlerError> {
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
}
