use crate::Result;
use crate::SonosDevice;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{channel, Receiver};

/// URN identifying Sonos ZonePlayer compatible products.
/// This is used internally by the `discover` function but is
/// made available to you in case you plan to implement your
/// own custom discovery functionality.
pub const SONOS_URN: &str = "urn:schemas-upnp-org:device:ZonePlayer:1";

/// Discover SonosDevices on the network, stopping once the specified
/// timeout is reached.
/// Returns a channel that will yield `SonosDevice` instances as responses
/// to discovery requests are detected.
/// Note that it is possible (likely) for duplicates to be returned.
pub async fn discover(timeout: Duration) -> Result<Receiver<SonosDevice>> {
    const MX: usize = 3;

    let timeout = if timeout.as_secs() as usize <= MX {
        Duration::from_secs(MX as u64 + 1)
    } else {
        timeout
    };

    let disco_packet = format!(
        "M-SEARCH * HTTP/1.1\r\n\
        HOST: 239.255.255.250:1900\r\n\
        MAN: ssdp:discover\r\n\
        MX: {MX}\r\n\
        ST: {SONOS_URN}\r\n\r\n"
    );
    const DEFAULT_SEARCH_TTL: u32 = 2;

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_multicast_ttl_v4(DEFAULT_SEARCH_TTL).ok();
    socket
        .send_to(disco_packet.as_bytes(), "239.255.255.250:1900")
        .await?;

    let deadline = tokio::time::Instant::now() + timeout;

    let (tx, rx) = channel(8);

    tokio::spawn(async move {
        let mut buf = [0u8; 2048];

        loop {
            match tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await {
                Ok(Ok((n_read, peer))) => {
                    let buf = &buf[0..n_read];
                    let buf = String::from_utf8_lossy(&buf);
                    log::trace!("DISCO: ({peer:?}) {buf}");
                    let mut headers: BTreeMap<String, String> = BTreeMap::new();
                    for line in buf.lines() {
                        let Some((name, value)) = line.split_once(':') else {
                            continue;
                        };

                        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
                    }
                    log::trace!("Headers: {headers:?}");

                    match (headers.get("st"), headers.get("location")) {
                        (Some(st), Some(url)) if st == SONOS_URN => {
                            if let Ok(url) = url.parse() {
                                if let Ok(device) = SonosDevice::from_url(url).await {
                                    if tx.send(device).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Err(err)) => {
                    log::error!("{err:#}");
                    break;
                }
                Err(_) => break,
            }
        }
    });

    Ok(rx)
}
