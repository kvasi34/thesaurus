use log::debug;
use tokio::io::{self, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

use crate::resp2::{self, RespValue};

mod del;
mod expire;
mod get;
mod persist;
mod ping;
mod set;
mod ttl;
mod unknown;

pub(crate) use del::handle_del_response;
pub(crate) use expire::handle_expire_response;
pub(crate) use get::handle_get_response;
pub(crate) use persist::handle_persist_response;
pub(crate) use ping::handle_ping_response;
pub(crate) use set::handle_set_response;
pub(crate) use ttl::handle_ttl_response;
pub(crate) use unknown::handle_unknown_command_response;

pub(crate) async fn send_response<R>(
    uuid: &Uuid,
    stream: &mut R,
    response_value: RespValue,
) -> io::Result<()>
where
    R: AsyncWrite + Unpin,
{
    let encoded_response = resp2::encode(&response_value);
    debug!("Handler {} sending: {:?}", uuid, response_value);
    stream.write_all(&encoded_response).await
}
