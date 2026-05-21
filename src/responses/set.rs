use log::debug;
use tokio::io::{self, AsyncWrite};
use uuid::Uuid;

use crate::resp2::RespValue;
use crate::store::Store;

pub(crate) async fn handle_set_response<R>(
    uuid: &Uuid,
    key: String,
    value: String,
    stream: &mut R,
    store: &Store,
) -> io::Result<()>
where
    R: AsyncWrite + Unpin,
{
    debug!("Handler {} writing key {}: {}", uuid, key, value);
    store.set(&key, value);

    let response_value = RespValue::SimpleString("OK".to_string());
    super::send_response(uuid, stream, response_value).await
}
