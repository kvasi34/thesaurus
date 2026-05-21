use log::warn;
use tokio::io::{self, AsyncWrite};
use uuid::Uuid;

use crate::resp2::RespValue;

pub(crate) async fn handle_unknown_command_response<R>(
    uuid: &Uuid,
    e: String,
    stream: &mut R,
) -> io::Result<()>
where
    R: AsyncWrite + Unpin,
{
    warn!("Handler {} received unknown command: {}", uuid, e);
    let response_value = RespValue::SimpleError(format!("ERR unknown command '{}'", e));
    super::send_response(uuid, stream, response_value).await
}
