use crate::SonosDevice;
use crate::{Error, Result};
use futures_util::stream::{Stream, StreamExt, TryStreamExt};
use ssdp_client::URN;
use std::time::Duration;

pub const SONOS_URN: URN = URN::device("schemas-upnp-org", "ZonePlayer", 1);

pub async fn discover(timeout: Duration) -> Result<impl Stream<Item = Result<SonosDevice>>> {
    const MX: usize = 3;
    let timeout = if timeout.as_secs() as usize <= MX {
        Duration::from_secs(MX as u64 + 1)
    } else {
        timeout
    };
    let ttl = None;

    Ok(ssdp_client::search(&SONOS_URN.into(), timeout, MX, ttl)
        .await?
        .map_err(Error::Ssdp)
        .map(|res| Ok(res?.location().parse()?))
        .and_then(SonosDevice::from_url))
}
