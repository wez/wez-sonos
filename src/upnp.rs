use crate::Error;
use instant_xml::FromXml;
use reqwest::{Method, Response, Url};
use std::net::IpAddr;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use url::Host;

const UPNP_DEVICE: &str = "urn:schemas-upnp-org:device-1-0";

#[derive(Debug, FromXml)]
#[xml(rename = "device", ns(UPNP_DEVICE))]
pub struct DeviceSpec {
    #[xml(rename = "friendlyName")]
    pub friendly_name: String,
    #[xml(rename = "deviceType")]
    pub device_type: String,
    #[xml(rename = "modelNumber")]
    pub model_number: Option<String>,
    #[xml(rename = "modelDescription")]
    pub model_description: Option<String>,
    #[xml(rename = "modelName")]
    pub model_name: Option<String>,
    #[xml(rename = "SSLPort")]
    pub ssl_port: Option<u16>,

    service_list: Option<ServiceList>,
    device_list: Option<DeviceList>,
}

impl DeviceSpec {
    pub fn parse_xml(xml: &str) -> crate::Result<Self> {
        let spec: Root = instant_xml::from_str(xml).map_err(|error| crate::Error::XmlParse {
            error,
            text: xml.to_string(),
        })?;
        Ok(spec.device)
    }

    pub fn services(&self) -> &[Service] {
        match &self.service_list {
            None => &[],
            Some(list) => &list.services,
        }
    }

    pub fn get_service(&self, service_type: &str) -> Option<&Service> {
        if let Some(s) = self
            .services()
            .iter()
            .find(|s| *s.service_type == *service_type)
        {
            return Some(s);
        }
        if let Some(dev) = &self.device_list {
            for d in dev.devices.iter() {
                if let Some(s) = d.get_service(service_type) {
                    return Some(s);
                }
            }
        }

        None
    }
}

#[derive(Debug, FromXml)]
#[xml(rename = "serviceList", ns(UPNP_DEVICE))]
struct ServiceList {
    pub services: Vec<Service>,
}

#[derive(Debug, FromXml)]
#[xml(rename = "deviceList", ns(UPNP_DEVICE))]
struct DeviceList {
    pub devices: Vec<DeviceSpec>,
}

#[derive(Debug, FromXml)]
#[xml(rename = "root", ns(UPNP_DEVICE))]
struct Root {
    device: DeviceSpec,
}

#[derive(Debug, FromXml)]
#[xml(rename = "service", ns(UPNP_DEVICE))]
pub struct Service {
    #[xml(rename = "serviceType")]
    pub service_type: String,
    #[xml(rename = "serviceId")]
    pub service_id: String,
    #[xml(rename = "controlURL")]
    pub control_url: String,
    #[xml(rename = "eventSubURL")]
    pub event_sub_url: String,
    #[xml(rename = "SCPDURL")]
    pub scpd_url: String,
}

impl Service {
    fn join_url(&self, base_url: &Url, url: &str) -> Url {
        match base_url.join(url) {
            Ok(url) => url,
            Err(err) => {
                log::error!("Cannot join {base_url} with {url}: {err:#}");
                url.parse().expect("URL to be valid")
            }
        }
    }

    pub fn control_url(&self, url: &Url) -> Url {
        self.join_url(url, &self.control_url)
    }

    pub fn event_sub_url(&self, url: &Url) -> Url {
        self.join_url(url, &self.event_sub_url)
    }

    /// The URL for the Service Control Protocol Description
    pub fn scpd_url(&self, url: &Url) -> Url {
        self.join_url(url, &self.scpd_url)
    }

    pub async fn subscribe<T: DecodeXml + 'static>(
        &self,
        url: &Url,
    ) -> crate::Result<EventStream<T>> {
        let sub_url = self.event_sub_url(url);

        // Figure out an appropriate local address to talk to
        // this device
        let host = url
            .host()
            .ok_or_else(|| Error::NoIpInDeviceUrl(url.clone()))?;
        let ip: IpAddr = match host {
            Host::Domain(_s) => return Err(Error::NoIpInDeviceUrl(url.clone())),
            Host::Ipv4(v4) => v4.into(),
            Host::Ipv6(v6) => v6.into(),
        };

        let probe = TcpStream::connect((ip, url.port().unwrap_or(80))).await?;
        let listener = TcpListener::bind((probe.local_addr()?.ip(), 0)).await?;
        let local = listener.local_addr()?;

        let response = reqwest::Client::new()
            .request(
                Method::from_bytes(b"SUBSCRIBE").expect("SUBSCRIBE to be a valid method"),
                sub_url.clone(),
            )
            .header("CALLBACK", format!("<http://{local}>"))
            .header("NT", "upnp:event")
            .header("TIMEOUT", format!("Second-{SUBSCRIPTION_TIMEOUT}"))
            .send()
            .await?;

        let response = Error::check_response(response).await?;

        log::trace!("response: {response:?}");

        let sid = response
            .headers()
            .get("sid")
            .ok_or(Error::SubscriptionFailedNoSid)?
            .to_str()
            .map_err(|_| Error::SubscriptionFailedNoSid)?
            .to_string();

        let body = response.text().await?;
        log::trace!("Got response: {body}");

        let (tx, rx) = channel(16);
        {
            let sid = sid.clone();
            let sub_url = sub_url.clone();
            tokio::spawn(async move { process_subscription(listener, tx, sid, sub_url).await });
        }

        Ok(EventStream { sid, rx, sub_url })
    }
}

const SUBSCRIPTION_TIMEOUT: u64 = 60;

async fn process_subscription<T: DecodeXml + 'static>(
    listener: TcpListener,
    tx: Sender<SubscriptionMessage<T>>,
    sid: String,
    sub_url: Url,
) -> crate::Result<()> {
    let mut deadline =
        tokio::time::Instant::now() + tokio::time::Duration::from_secs(SUBSCRIPTION_TIMEOUT - 10);
    loop {
        match tokio::time::timeout_at(deadline, listener.accept()).await {
            Ok(Ok((client, _addr))) => {
                let tx = tx.clone();
                tokio::spawn(async move { handle_subscription_request(client, tx).await });
            }
            Ok(Err(err)) => {
                log::error!("accept failed: {err:#}");
                return Ok(());
            }
            Err(_) => {
                log::debug!("time to renew!");
                // Time to renew subscription
                let renew = match dbg!(tx.try_send(SubscriptionMessage::Ping)) {
                    Ok(_) | Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => true,
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                        // It's dead; don't bother renewing
                        false
                    }
                };

                dbg!(renew_or_cancel_sub(&sub_url, renew, &sid).await)?;

                if renew {
                    deadline = tokio::time::Instant::now()
                        + tokio::time::Duration::from_secs(SUBSCRIPTION_TIMEOUT - 10);
                } else {
                    return Ok(());
                }
            }
        }
    }
}

async fn handle_subscription_request<T: DecodeXml>(
    mut client: TcpStream,
    tx: Sender<SubscriptionMessage<T>>,
) -> crate::Result<()> {
    let mut reqbuf = vec![];
    let mut buf = [0u8; 4096];

    while let Ok(len) = client.read(&mut buf).await {
        reqbuf.extend_from_slice(&buf[0..len]);

        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut req = httparse::Request::new(&mut headers);

        match req.parse(&reqbuf) {
            Err(err) => {
                log::error!("Error parsing request: {err:#}");
                break;
            }
            Ok(httparse::Status::Partial) => continue,
            Ok(httparse::Status::Complete(body_start)) => {
                // It's only *maybe* complete; check the content-length
                // vs. the data in the buffer
                if let Some(cl) = req
                    .headers
                    .iter()
                    .find(|h| h.name.eq_ignore_ascii_case("Content-Length"))
                {
                    match std::str::from_utf8(cl.value)
                        .ok()
                        .and_then(|s| s.parse::<usize>().ok())
                    {
                        Some(cl) => {
                            let avail = reqbuf.len() - body_start;
                            if avail < cl {
                                // We need more data
                                continue;
                            }
                        }
                        None => {
                            log::error!("Invalid header: {cl:?}");
                            break;
                        }
                    }
                }
                let body = String::from_utf8_lossy(&reqbuf[body_start..]).to_string();

                log::trace!("{req:#?}");
                log::trace!("{body}");

                match T::decode_xml(&body) {
                    Ok(event) => {
                        if let Err(err) = tx.send(SubscriptionMessage::Event(event)).await {
                            log::error!("Channel is dead {err:#}");
                            return Ok(());
                        }
                    }
                    Err(err) => {
                        log::error!("Failed to parse PropertySet: {err:#} from {body}");
                    }
                }

                break;
            }
        }
    }
    Ok(())
}

async fn renew_or_cancel_sub(sub_url: &Url, subscribe: bool, sid: &str) -> crate::Result<Response> {
    let mut request = reqwest::Client::new()
        .request(
            Method::from_bytes(if subscribe {
                b"SUBSCRIBE"
            } else {
                b"UNSUBSCRIBE"
            })
            .expect("SUBSCRIBE to be a valid method"),
            sub_url.clone(),
        )
        .header("SID", sid);
    if subscribe {
        request = request.header("TIMEOUT", format!("Second-{SUBSCRIPTION_TIMEOUT}"));
    }
    let response = request.send().await?;

    let response = Error::check_response(response).await?;

    Ok(response)
}

enum SubscriptionMessage<T> {
    Ping,
    Event(T),
}

/// A helper trait for parsing a uPNP event stream into
/// a more ergonomic Rust type
pub trait DecodeXml: Send {
    fn decode_xml(xml: &str) -> crate::Result<Self>
    where
        Self: Sized;
}

/// Manages a live subscription to an event stream for a service.
/// While this object is live, the event stream will be renewed
/// every minute.
/// The stream isn't automatically cancelled on Drop because there
/// is no async-Drop, but you can call the `unsubscribe` method
/// to explicitly cancel it.
/// The stream dispatching machinery has liveness checking that will ping
/// the internal receiver and will cancel the subscription after about
/// a minute or so of the EventStream being dropped.
pub struct EventStream<T: DecodeXml> {
    rx: Receiver<SubscriptionMessage<T>>,
    sid: String,
    sub_url: Url,
}

impl<T: DecodeXml> EventStream<T> {
    /// Receives the next event from the stream
    pub async fn recv(&mut self) -> Option<T> {
        loop {
            let msg = self.rx.recv().await?;
            match msg {
                SubscriptionMessage::Ping => {}
                SubscriptionMessage::Event(v) => {
                    return Some(v);
                }
            }
        }
    }

    /// Explicitly cancel the subscription
    pub async fn unsubscribe(self) {
        renew_or_cancel_sub(&self.sub_url, false, &self.sid)
            .await
            .ok();
    }
}

pub(crate) const UPNP_EVENT: &str = "urn:schemas-upnp-org:event-1-0";

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_property_set() {
        let event= crate::av_transport::AVTransportEvent ::decode_xml(r#"<e:propertyset xmlns:e="urn:schemas-upnp-org:event-1-0"><e:property><LastChange>something</LastChange></e:property></e:propertyset>"#).unwrap();
        k9::snapshot!(
            event,
            r#"
AVTransportEvent {
    last_change: Some(
        "something",
    ),
}
"#
        );
    }

    #[test]
    fn parse_device_spec() {
        let spec_text = include_str!("../data/device_spec.xml");
        let spec: Root = instant_xml::from_str(&spec_text).unwrap();
        k9::snapshot!(
            spec,
            r#"
Root {
    device: DeviceSpec {
        friendly_name: "192.168.1.157 - Sonos Port - RINCON_XXX",
        device_type: "urn:schemas-upnp-org:device:ZonePlayer:1",
        model_number: Some(
            "S23",
        ),
        model_description: Some(
            "Sonos Port",
        ),
        model_name: Some(
            "Sonos Port",
        ),
        ssl_port: Some(
            1443,
        ),
        service_list: Some(
            ServiceList {
                services: [
                    Service {
                        service_type: "urn:schemas-upnp-org:service:AlarmClock:1",
                        service_id: "urn:upnp-org:serviceId:AlarmClock",
                        control_url: "/AlarmClock/Control",
                        event_sub_url: "/AlarmClock/Event",
                        scpd_url: "/xml/AlarmClock1.xml",
                    },
                    Service {
                        service_type: "urn:schemas-upnp-org:service:MusicServices:1",
                        service_id: "urn:upnp-org:serviceId:MusicServices",
                        control_url: "/MusicServices/Control",
                        event_sub_url: "/MusicServices/Event",
                        scpd_url: "/xml/MusicServices1.xml",
                    },
                    Service {
                        service_type: "urn:schemas-upnp-org:service:AudioIn:1",
                        service_id: "urn:upnp-org:serviceId:AudioIn",
                        control_url: "/AudioIn/Control",
                        event_sub_url: "/AudioIn/Event",
                        scpd_url: "/xml/AudioIn1.xml",
                    },
                    Service {
                        service_type: "urn:schemas-upnp-org:service:DeviceProperties:1",
                        service_id: "urn:upnp-org:serviceId:DeviceProperties",
                        control_url: "/DeviceProperties/Control",
                        event_sub_url: "/DeviceProperties/Event",
                        scpd_url: "/xml/DeviceProperties1.xml",
                    },
                    Service {
                        service_type: "urn:schemas-upnp-org:service:SystemProperties:1",
                        service_id: "urn:upnp-org:serviceId:SystemProperties",
                        control_url: "/SystemProperties/Control",
                        event_sub_url: "/SystemProperties/Event",
                        scpd_url: "/xml/SystemProperties1.xml",
                    },
                    Service {
                        service_type: "urn:schemas-upnp-org:service:ZoneGroupTopology:1",
                        service_id: "urn:upnp-org:serviceId:ZoneGroupTopology",
                        control_url: "/ZoneGroupTopology/Control",
                        event_sub_url: "/ZoneGroupTopology/Event",
                        scpd_url: "/xml/ZoneGroupTopology1.xml",
                    },
                    Service {
                        service_type: "urn:schemas-upnp-org:service:GroupManagement:1",
                        service_id: "urn:upnp-org:serviceId:GroupManagement",
                        control_url: "/GroupManagement/Control",
                        event_sub_url: "/GroupManagement/Event",
                        scpd_url: "/xml/GroupManagement1.xml",
                    },
                    Service {
                        service_type: "urn:schemas-tencent-com:service:QPlay:1",
                        service_id: "urn:tencent-com:serviceId:QPlay",
                        control_url: "/QPlay/Control",
                        event_sub_url: "/QPlay/Event",
                        scpd_url: "/xml/QPlay1.xml",
                    },
                ],
            },
        ),
        device_list: Some(
            DeviceList {
                devices: [
                    DeviceSpec {
                        friendly_name: "192.168.1.157 - Sonos Port Media Server - RINCON_XXX",
                        device_type: "urn:schemas-upnp-org:device:MediaServer:1",
                        model_number: Some(
                            "S23",
                        ),
                        model_description: Some(
                            "Sonos Port Media Server",
                        ),
                        model_name: Some(
                            "Sonos Port",
                        ),
                        ssl_port: None,
                        service_list: Some(
                            ServiceList {
                                services: [
                                    Service {
                                        service_type: "urn:schemas-upnp-org:service:ContentDirectory:1",
                                        service_id: "urn:upnp-org:serviceId:ContentDirectory",
                                        control_url: "/MediaServer/ContentDirectory/Control",
                                        event_sub_url: "/MediaServer/ContentDirectory/Event",
                                        scpd_url: "/xml/ContentDirectory1.xml",
                                    },
                                    Service {
                                        service_type: "urn:schemas-upnp-org:service:ConnectionManager:1",
                                        service_id: "urn:upnp-org:serviceId:ConnectionManager",
                                        control_url: "/MediaServer/ConnectionManager/Control",
                                        event_sub_url: "/MediaServer/ConnectionManager/Event",
                                        scpd_url: "/xml/ConnectionManager1.xml",
                                    },
                                ],
                            },
                        ),
                        device_list: None,
                    },
                    DeviceSpec {
                        friendly_name: "Some Room - Sonos Port Media Renderer - RINCON_XXX",
                        device_type: "urn:schemas-upnp-org:device:MediaRenderer:1",
                        model_number: Some(
                            "S23",
                        ),
                        model_description: Some(
                            "Sonos Port Media Renderer",
                        ),
                        model_name: Some(
                            "Sonos Port",
                        ),
                        ssl_port: None,
                        service_list: Some(
                            ServiceList {
                                services: [
                                    Service {
                                        service_type: "urn:schemas-upnp-org:service:RenderingControl:1",
                                        service_id: "urn:upnp-org:serviceId:RenderingControl",
                                        control_url: "/MediaRenderer/RenderingControl/Control",
                                        event_sub_url: "/MediaRenderer/RenderingControl/Event",
                                        scpd_url: "/xml/RenderingControl1.xml",
                                    },
                                    Service {
                                        service_type: "urn:schemas-upnp-org:service:ConnectionManager:1",
                                        service_id: "urn:upnp-org:serviceId:ConnectionManager",
                                        control_url: "/MediaRenderer/ConnectionManager/Control",
                                        event_sub_url: "/MediaRenderer/ConnectionManager/Event",
                                        scpd_url: "/xml/ConnectionManager1.xml",
                                    },
                                    Service {
                                        service_type: "urn:schemas-upnp-org:service:AVTransport:1",
                                        service_id: "urn:upnp-org:serviceId:AVTransport",
                                        control_url: "/MediaRenderer/AVTransport/Control",
                                        event_sub_url: "/MediaRenderer/AVTransport/Event",
                                        scpd_url: "/xml/AVTransport1.xml",
                                    },
                                    Service {
                                        service_type: "urn:schemas-sonos-com:service:Queue:1",
                                        service_id: "urn:sonos-com:serviceId:Queue",
                                        control_url: "/MediaRenderer/Queue/Control",
                                        event_sub_url: "/MediaRenderer/Queue/Event",
                                        scpd_url: "/xml/Queue1.xml",
                                    },
                                    Service {
                                        service_type: "urn:schemas-upnp-org:service:GroupRenderingControl:1",
                                        service_id: "urn:upnp-org:serviceId:GroupRenderingControl",
                                        control_url: "/MediaRenderer/GroupRenderingControl/Control",
                                        event_sub_url: "/MediaRenderer/GroupRenderingControl/Event",
                                        scpd_url: "/xml/GroupRenderingControl1.xml",
                                    },
                                    Service {
                                        service_type: "urn:schemas-upnp-org:service:VirtualLineIn:1",
                                        service_id: "urn:upnp-org:serviceId:VirtualLineIn",
                                        control_url: "/MediaRenderer/VirtualLineIn/Control",
                                        event_sub_url: "/MediaRenderer/VirtualLineIn/Event",
                                        scpd_url: "/xml/VirtualLineIn1.xml",
                                    },
                                ],
                            },
                        ),
                        device_list: None,
                    },
                ],
            },
        ),
    },
}
"#
        );
    }
}
