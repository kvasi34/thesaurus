use log::trace;
use tokio::io::{self, AsyncWrite};
use uuid::Uuid;

use crate::resp2::RespValue;
use crate::store::Store;

pub(crate) async fn handle_persist_response<R>(
    uuid: &Uuid,
    key: String,
    stream: &mut R,
    store: &Store,
) -> io::Result<()>
where
    R: AsyncWrite + Unpin,
{
    trace!("Handler {} persisting key {}", uuid, key);
    let removed = store.persist(&key);
    let response_value = RespValue::Integer(removed as i64);
    super::send_response(uuid, stream, response_value).await
}
