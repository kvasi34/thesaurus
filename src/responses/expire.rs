use log::debug;
use std::time::{Duration, Instant};
use tokio::io::{self, AsyncWrite};
use uuid::Uuid;

use crate::resp2::RespValue;
use crate::store::Store;

pub(crate) async fn handle_expire_response<R>(
    uuid: &Uuid,
    key: String,
    seconds: u64,
    stream: &mut R,
    store: &Store,
) -> io::Result<()>
where
    R: AsyncWrite + Unpin,
{
    debug!(
        "Handler {} expiring key {} with {} seconds",
        uuid, key, seconds
    );

    let expiration = Instant::now().checked_add(Duration::from_secs(seconds));
    // None means that an overflow occured during the addition; reply with a generic error message
    if expiration.is_none() {
        return super::send_response(
            uuid,
            stream,
            RespValue::SimpleError("ERR invalid expire time in 'expire' command".to_string()),
        )
        .await;
    }

    let response_value = RespValue::Integer(store.set_ttl(&key, expiration.unwrap()) as i64);
    super::send_response(uuid, stream, response_value).await
}
