use log::debug;
use tokio::io::{self, AsyncWrite};
use uuid::Uuid;

use crate::resp2::RespValue;
use crate::store::Store;

pub(crate) async fn handle_del_response<R>(
    uuid: &Uuid,
    keys: Vec<String>,
    stream: &mut R,
    store: &Store,
) -> io::Result<()>
where
    R: AsyncWrite + Unpin,
{
    debug!("Handler {} deleting keys: {:?}", uuid, keys);
    let mut deleted_key_count: i64 = 0;
    for key in keys.iter() {
        deleted_key_count += store.delete(key) as i64;
    }

    let response_value = RespValue::Integer(deleted_key_count);
    super::send_response(uuid, stream, response_value).await
}
