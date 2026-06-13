use log::{debug, error, warn};
use tokio::io::{self, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::aof::AofWriter;
use crate::{
    command::Command,
    errors::{HandlerError, RespError},
    executor::Executor,
    resp2::{self, RespValue},
};

/// Carries the data needed to convert a relative-TTL command to `PEXPIREAT` before AOF persistence.
enum ExpireAofConversion {
    Expire { key: String, seconds: u64 },
    PExpire { key: String, milliseconds: u64 },
}

/// TCP socket connection handler.
#[derive(Debug)]
pub struct Handler {
    uuid: Uuid,
    socket: TcpStream,
    executor: Executor,
    aof_writer: Option<AofWriter>,
}

impl Handler {
    /// Constructor for `Handler` struct.
    pub fn new(socket: TcpStream, executor: Executor, aof_writer: Option<AofWriter>) -> Self {
        Handler {
            uuid: Uuid::new_v4(),
            socket,
            executor,
            aof_writer,
        }
    }

    /// Reads and dispatches RESP2 commands from the client connection in a loop.
    ///
    /// Runs until the client closes the connection or an unrecoverable decode
    /// error occurs. Each iteration decodes one RESP2 message, parses it into
    /// a [`Command`], executes it via [`Executor`], and writes the RESP2
    /// response back to the socket.
    pub async fn run_handler(mut self) -> io::Result<()> {
        debug!("Handler {} started", self.uuid);

        let mut stream = BufReader::new(self.socket);
        loop {
            // Read from the socket and decode the RESP value into a `RespValue` struct
            let resp_value = match resp2::decode_async(&mut stream).await {
                Ok(v) => v,
                Err(RespError::UnexpectedEof) => {
                    debug!("Handler {} client disconnected", self.uuid);
                    break;
                }
                Err(e) => {
                    warn!("Handler {} decode error: {}", self.uuid, e);
                    return Err(e.into());
                }
            };

            // Convert the `RespValue` into a `Command` struct
            let cmd = Command::from_resp2(&resp_value);
            debug!("Handler {} received command: {:?}", self.uuid, cmd);

            // Capture the write flag before cmd is consumed by the match statement below
            let is_write_cmd = cmd.as_ref().is_ok_and(|c| c.is_write());

            // Capture relative-TTL commands before cmd is consumed; AOF persists them as PEXPIREAT.
            let expire_info = is_write_cmd.then(|| {
                cmd.as_ref().ok().and_then(|c| match c {
                    Command::Expire { key, seconds } => Some(ExpireAofConversion::Expire {
                        key: key.clone(),
                        seconds: *seconds,
                    }),
                    Command::PExpire { key, milliseconds } => Some(ExpireAofConversion::PExpire {
                        key: key.clone(),
                        milliseconds: *milliseconds,
                    }),
                    _ => None,
                })
            });

            // Execute the command using the `Executor` and generate the appropriate response
            // The `Executor` instance is responsible for both executing the command at the `Store` and generating a `RespValue` response
            let response = match cmd {
                Ok(cmd) => self.executor.execute(&cmd),
                Err(HandlerError::UnknownCommand(s)) => {
                    warn!("Handler {} received unknown command: {}", self.uuid, s);
                    RespValue::SimpleError(format!("ERR unknown command '{}'", s))
                }
                Err(e) => {
                    warn!("Handler {} command parse error: {}", self.uuid, e);
                    RespValue::SimpleError(e.to_string())
                }
            };

            // Call the AOF writer if the `appendonly` configuration is set to `on` and the command executed succesfully; no error
            if is_write_cmd
                && !matches!(response, RespValue::SimpleError(_))
                && !matches!(response, RespValue::Integer(0)) // DEL, EXPIRE and PERSIST return Integer(0) instead of SimpleError
                && let Some(writer) = self.aof_writer.as_mut()
            {
                // Re-encode the command before writing to the AOF
                let cmd_bytes = match expire_info.unwrap() {
                    Some(ExpireAofConversion::Expire { key, seconds }) => {
                        resp2::convert_expire_to_pexpireat(key, seconds)
                    }
                    Some(ExpireAofConversion::PExpire { key, milliseconds }) => {
                        resp2::convert_pexpire_to_pexpireat(key, milliseconds)
                    }
                    None => resp2::encode(&resp_value),
                };

                if let Err(e) = writer.append(&cmd_bytes) {
                    error!("Handler {} failed to write to AOF: {}", self.uuid, e);
                }
            }

            send_response(&self.uuid, &mut stream, response).await?;
        }

        debug!("Handler {} stopped", self.uuid);
        Ok(())
    }
}

/// Encodes `response_value` and writes it to `stream`.
async fn send_response<W>(uuid: &Uuid, stream: &mut W, response_value: RespValue) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let encoded = resp2::encode(&response_value);
    debug!("Handler {} sending: {:?}", uuid, response_value);
    stream.write_all(&encoded).await
}
