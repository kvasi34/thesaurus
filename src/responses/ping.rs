use tokio::io::{self, AsyncWrite};
use uuid::Uuid;

use crate::resp2::RespValue;

pub(crate) async fn handle_ping_response<R>(
    uuid: &Uuid,
    message: Option<String>,
    stream: &mut R,
) -> io::Result<()>
where
    R: AsyncWrite + Unpin,
{
    let response_value = match message {
        Some(s) => RespValue::BulkString(Some(s)),
        None => RespValue::SimpleString("PONG".to_string()),
    };

    super::send_response(uuid, stream, response_value).await
}
