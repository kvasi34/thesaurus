use log::trace;
use tokio::io::{self, AsyncWrite};
use uuid::Uuid;

use crate::resp2::RespValue;
use crate::store::Store;

pub(crate) async fn handle_exists_response<R>(
    uuid: &Uuid,
    keys: Vec<String>,
    stream: &mut R,
    store: &Store,
) -> io::Result<()>
where
    R: AsyncWrite + Unpin,
{
    let mut existing_keys_count: i64 = 0;
    for key in keys.iter() {
        trace!("Handler {} getting key {}", uuid, key);
        let key_exists = store.exists(key);
        existing_keys_count += key_exists as i64;
        trace!(
            "Handler {} key {} {} exist",
            uuid,
            key,
            if !key_exists { "does not" } else { "" }
        );
    }

    let response_value = RespValue::Integer(existing_keys_count);
    super::send_response(uuid, stream, response_value).await
}
