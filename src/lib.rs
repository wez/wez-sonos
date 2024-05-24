use instant_xml::{FromXmlOwned, ToXml};
use reqwest::{StatusCode, Url};
use std::net::Ipv4Addr;
use thiserror::Error;

mod didl;
mod discovery;
mod generated;
mod upnp;
mod xmlutil;
mod zone;

pub use didl::*;
pub use discovery::*;
pub use generated::*;
pub use upnp::*;
pub use xmlutil::DecodeXmlString;
pub use zone::*;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("XML Error: {0}")]
    Xml(#[from] instant_xml::Error),
    #[error("XML Error: {error:#} while parsing {text}")]
    XmlParse {
        error: instant_xml::Error,
        text: String,
    },
    #[error("Service {0:?} is not supported by this device")]
    UnsupportedService(String),
    #[error("Invalid URI: {0:#?}")]
    InvalidUri(#[from] url::ParseError),
    #[error("Reqwest Error: {0:#?}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Failed Request: {status:?} {body}")]
    FailedRequest {
        status: StatusCode,
        body: String,
        headers: reqwest::header::HeaderMap,
    },
    #[error("Device has no name!?")]
    NoName,
    #[error("I/O Error: {0:#}")]
    Io(#[from] std::io::Error),
    #[error("Invalid enum variant value")]
    InvalidEnumVariantValue,
    #[error("Room {0} not found")]
    RoomNotFound(String),
    #[error("Cannot find IP from device URL! {0:?}")]
    NoIpInDeviceUrl(Url),
    #[error("Subscription failed because SID header is missing")]
    SubscriptionFailedNoSid,
    #[error("TrackMetaData list is empty!?")]
    EmptyTrackMetaData,
    #[error("TrackMetaData has multiple items but expect a single item")]
    MoreThanOneTrackMetaData,
    #[error("LastChange format unexpected {0}")]
    LastChangeFormatUnexpected(String),
}

impl Error {
    pub async fn with_failed_http_response(response: reqwest::Response) -> Error {
        let status = response.status();
        let headers = response.headers().clone();
        let body = match response.bytes().await {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(err) => format!("Failed to retrieve body from failed request: {err:#}"),
        };

        return Error::FailedRequest {
            status,
            body,
            headers,
        };
    }

    pub async fn check_response(response: reqwest::Response) -> Result<reqwest::Response> {
        let status = response.status();
        if !status.is_success() {
            Err(Self::with_failed_http_response(response).await)
        } else {
            Ok(response)
        }
    }
}

#[derive(Debug)]
pub struct SonosDevice {
    url: Url,
    device: DeviceSpec,
}

impl SonosDevice {
    /// Constructs a SonosDevice from the supplied IP Address.
    /// Validates that the device is actually a Sonos device
    /// before returning successfully.
    pub async fn from_ip(addr: Ipv4Addr) -> Result<Self> {
        Self::from_url(format!("http://{addr}:1400/xml/device_description.xml").parse()?).await
    }

    /// Resolves the SonosDevice whose name is equal to the provided
    /// name.  If no matching device is found within a reasonably
    /// short, unspecified, implementation-defined timeout, then
    /// an `Error::RoomNotFound` is produced.
    pub async fn for_room(room_name: &str) -> Result<Self> {
        let mut rx = discover(std::time::Duration::from_secs(15)).await?;
        while let Some(device) = rx.recv().await {
            if let Ok(name) = device.name().await {
                if name == room_name {
                    return Ok(device);
                }
            }
        }

        Err(Error::RoomNotFound(room_name.to_string()))
    }

    /// Constructs a SonosDevice from the supplied URL, which must
    /// be the device_description.xml URL for that device.
    /// Validates that the device is actually a Sonos device
    /// before returning successfully.
    pub async fn from_url(url: Url) -> Result<Self> {
        let response = reqwest::get(url.clone()).await?;

        let response = Error::check_response(response).await?;
        let body = response.text().await?;
        let device = DeviceSpec::parse_xml(&body)?;

        Ok(Self { url, device })
    }

    /// Returns the room/zone name of the device
    pub async fn name(&self) -> Result<String> {
        let attr = self.get_zone_attributes().await?;
        attr.current_zone_name.ok_or(Error::NoName)
    }

    /// Returns information about the zone to which this device belongs
    pub async fn get_zone_group_state(&self) -> Result<Vec<ZoneGroup>> {
        let state = <Self as ZoneGroupTopology>::get_zone_group_state(self).await?;
        Ok(match state.zone_group_state {
            Some(state) => state.0.groups,
            None => vec![],
        })
    }

    /// Stops playback
    pub async fn stop(&self) -> Result<()> {
        <Self as AVTransport>::stop(self, Default::default()).await
    }

    /// Begin playback
    pub async fn play(&self) -> Result<()> {
        <Self as AVTransport>::play(
            self,
            av_transport::PlayRequest {
                instance_id: 0,
                speed: "1".to_string(),
            },
        )
        .await
    }

    /// Clears the queue
    pub async fn queue_clear(&self) -> Result<()> {
        <Self as AVTransport>::remove_all_tracks_from_queue(self, Default::default()).await
    }

    pub async fn set_play_mode(&self, new_play_mode: CurrentPlayMode) -> Result<()> {
        <Self as AVTransport>::set_play_mode(
            self,
            av_transport::SetPlayModeRequest {
                instance_id: 0,
                new_play_mode,
            },
        )
        .await
    }

    pub async fn set_av_transport_uri(
        &self,
        uri: &str,
        metadata: Option<TrackMetaData>,
    ) -> Result<()> {
        <Self as AVTransport>::set_av_transport_uri(
            self,
            av_transport::SetAvTransportUriRequest {
                instance_id: 0,
                current_uri: uri.to_string(),
                current_uri_meta_data: metadata
                    .map(|m| m.to_didl_string())
                    .unwrap_or_else(String::new),
            },
        )
        .await
    }

    pub async fn queue_prepend(
        &self,
        uri: &str,
        metadata: Option<TrackMetaData>,
    ) -> Result<av_transport::AddUriToQueueResponse> {
        <Self as AVTransport>::add_uri_to_queue(
            self,
            av_transport::AddUriToQueueRequest {
                instance_id: 0,
                enqueued_uri: uri.to_string(),
                enqueued_uri_meta_data: metadata
                    .map(|m| m.to_didl_string())
                    .unwrap_or_else(String::new),
                desired_first_track_number_enqueued: 0,
                enqueue_as_next: true,
            },
        )
        .await
    }

    pub async fn queue_append(
        &self,
        uri: &str,
        metadata: Option<TrackMetaData>,
    ) -> Result<av_transport::AddUriToQueueResponse> {
        <Self as AVTransport>::add_uri_to_queue(
            self,
            av_transport::AddUriToQueueRequest {
                instance_id: 0,
                enqueued_uri: uri.to_string(),
                enqueued_uri_meta_data: metadata
                    .map(|m| m.to_didl_string())
                    .unwrap_or_else(String::new),
                desired_first_track_number_enqueued: 0,
                enqueue_as_next: false,
            },
        )
        .await
    }

    pub async fn queue_browse(
        &self,
        starting_index: u32,
        requested_count: u32,
    ) -> Result<Vec<TrackMetaData>> {
        let result = <Self as Queue>::browse(
            self,
            queue::BrowseRequest {
                queue_id: 0,
                starting_index,
                requested_count,
            },
        )
        .await?;

        match result.result {
            Some(list) => Ok(list.into_inner().tracks),
            None => Ok(vec![]),
        }
    }
}

const SOAP_ENCODING: &str = "http://schemas.xmlsoap.org/soap/encoding/";
const SOAP_ENVELOPE: &str = "http://schemas.xmlsoap.org/soap/envelope/";

mod soap {
    use super::SOAP_ENVELOPE;
    use instant_xml::ToXml;

    #[derive(Debug, Eq, PartialEq, ToXml)]
    pub struct Unit;

    #[derive(Debug, Eq, PartialEq, ToXml)]
    #[xml(rename="s:Envelope", ns("", s = SOAP_ENVELOPE))]
    pub struct Envelope<T: ToXml> {
        #[xml(attribute, rename = "s:encodingStyle")]
        pub encoding_style: &'static str,
        pub body: Body<T>,
    }

    #[derive(Debug, Eq, PartialEq, ToXml)]
    #[xml(rename = "s:Body")]
    pub struct Body<T: ToXml> {
        pub payload: T,
    }
}

mod soap_resp {
    use super::SOAP_ENVELOPE;
    use instant_xml::FromXml;

    #[derive(Debug, Eq, PartialEq, FromXml)]
    #[xml(ns(SOAP_ENVELOPE))]
    pub struct Envelope<T> {
        #[xml(rename = "encodingStyle", attribute, ns(SOAP_ENVELOPE))]
        pub encoding_style: String,
        pub body: Body<T>,
    }

    #[derive(Debug, Eq, PartialEq, FromXml)]
    #[xml(ns(SOAP_ENVELOPE))]
    pub struct Body<T> {
        pub payload: T,
    }
}

/// Special case for decoding (), as instant_xml considers the empty
/// body in the `soap_resp::Body<T>` case to be an error
mod soap_empty_resp {
    use super::SOAP_ENVELOPE;
    use instant_xml::FromXml;

    #[derive(Debug, Eq, PartialEq, FromXml)]
    #[xml(ns(SOAP_ENVELOPE))]
    pub struct Envelope {
        #[xml(rename = "encodingStyle", attribute, ns(SOAP_ENVELOPE))]
        pub encoding_style: String,
        pub body: Body,
    }

    #[derive(Debug, Eq, PartialEq, FromXml)]
    #[xml(ns(SOAP_ENVELOPE))]
    pub struct Body {}
}

/// This trait decodes a SOAP response envelope into Self
pub trait DecodeSoapResponse {
    /// xml is a complete Soap `<Envelope>` element.
    /// This method decodes and returns Self from that Envelope.
    fn decode_soap_xml(xml: &str) -> Result<Self>
    where
        Self: Sized;
}

impl DecodeSoapResponse for () {
    fn decode_soap_xml(xml: &str) -> Result<()> {
        // Verify that it parses, but discard because it has no
        // useful content for us
        let _envelope: soap_empty_resp::Envelope = instant_xml::from_str(xml)?;
        Ok(())
    }
}

impl SonosDevice {
    pub fn device_spec(&self) -> &DeviceSpec {
        &self.device
    }

    pub async fn subscribe_helper<T: DecodeXml + 'static>(
        &self,
        service: &str,
    ) -> Result<EventStream<T>> {
        let service = self
            .device
            .get_service(service)
            .ok_or_else(|| Error::UnsupportedService(service.to_string()))?;
        service.subscribe(&self.url).await
    }

    /// This is a low level helper function for performing a SOAP Action
    /// request. You most likely want to use one of the methods
    /// implemented by the various service traits instead of this.
    pub async fn action<REQ: ToXml, RESP>(
        &self,
        service: &str,
        action: &str,
        payload: REQ,
    ) -> Result<RESP>
    where
        RESP: FromXmlOwned + std::fmt::Debug + DecodeSoapResponse,
    {
        let service = self
            .device
            .get_service(service)
            .ok_or_else(|| Error::UnsupportedService(service.to_string()))?;

        let envelope = soap::Envelope {
            encoding_style: SOAP_ENCODING,
            body: soap::Body { payload },
        };

        let body = instant_xml::to_string(&envelope)?;
        log::trace!("Sending: {body}");

        let soap_action = format!("\"{}#{action}\"", service.service_type);
        let url = service.control_url(&self.url);

        let response = reqwest::Client::new()
            .post(url)
            .header("CONTENT-TYPE", "text/xml; charset=\"utf-8\"")
            .header("SOAPAction", soap_action)
            .body::<String>(body.into())
            .send()
            .await?;

        let response = Error::check_response(response).await?;

        let body = response.text().await?;
        log::trace!("Got response: {body}");

        RESP::decode_soap_xml(&body)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_xml() {
        use crate::av_transport::StopRequest;
        let stop = StopRequest { instance_id: 32 };
        k9::snapshot!(
            instant_xml::to_string(&stop).unwrap(),
            r#"<Stop xmlns="urn:schemas-upnp-org:service:AVTransport:1"><InstanceID xmlns="">32</InstanceID></Stop>"#
        );
    }

    #[test]
    fn test_soap_envelope() {
        use crate::av_transport::StopRequest;

        let action = soap::Envelope {
            encoding_style: crate::SOAP_ENCODING,
            body: soap::Body {
                payload: StopRequest { instance_id: 0 },
            },
        };

        k9::snapshot!(
            instant_xml::to_string(&action).unwrap(),
            r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/"><s:Body><Stop xmlns="urn:schemas-upnp-org:service:AVTransport:1"><InstanceID xmlns="">0</InstanceID></Stop></s:Body></s:Envelope>"#
        );
    }
}
