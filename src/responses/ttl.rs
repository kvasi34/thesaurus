use log::trace;
use std::time::Instant;
use tokio::io::{self, AsyncWrite};
use uuid::Uuid;

use crate::resp2::RespValue;
use crate::store::Store;

pub(crate) async fn handle_ttl_response<R>(
    uuid: &Uuid,
    key: String,
    stream: &mut R,
    store: &Store,
) -> io::Result<()>
where
    R: AsyncWrite + Unpin,
{
    trace!("Handler {} getting TTL for key {}", uuid, key);
    let expiry_instant = store.get_ttl(&key);
    trace!(
        "Handler {} got TTL for key {} with expiry {:?}",
        uuid, key, expiry_instant
    );

    let response_value = match expiry_instant {
        // Get the TTL as seconds by comparing the expiry instant to the current instant
        Some(expiry) => match expiry.checked_duration_since(Instant::now()) {
            Some(remaining) => RespValue::Integer(remaining.as_secs() as i64),
            None => RespValue::Integer(-2), // The key has expired; treat as a missing key
        },
        // No expiry entry; check if the key exists in the store
        None => RespValue::Integer(if store.exists(&key) { -1 } else { -2 }),
    };
    super::send_response(uuid, stream, response_value).await
}
