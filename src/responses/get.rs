use log::trace;
use tokio::io::{self, AsyncWrite};
use uuid::Uuid;

use crate::resp2::RespValue;
use crate::store::Store;

pub(crate) async fn handle_get_response<R>(
    uuid: &Uuid,
    key: String,
    stream: &mut R,
    store: &Store,
) -> io::Result<()>
where
    R: AsyncWrite + Unpin,
{
    trace!("Handler {} getting key {}", uuid, key);
    let stored_value = store.get(&key);
    trace!(
        "Handler {} got key {} with value {:?}",
        uuid, key, stored_value
    );

    let response_value = RespValue::BulkString(stored_value);
    super::send_response(uuid, stream, response_value).await
}
