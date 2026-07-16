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
}
