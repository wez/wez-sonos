use crate::upnp::DeviceSpec;
use instant_xml::FromXmlOwned;
use instant_xml::ToXml;
use reqwest::StatusCode;
use reqwest::Url;
use std::net::Ipv4Addr;
use thiserror::Error;

mod discovery;
mod generated;
mod upnp;
mod zone;

pub use discovery::*;
pub use generated::*;
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
    FailedRequest { status: StatusCode, body: String },
    #[error("Device has no name!?")]
    NoName,
    #[error("I/O Error: {0:#}")]
    Io(#[from] std::io::Error),
    #[error("Invalid enum variant value")]
    InvalidEnumVariantValue,
}

#[derive(Debug)]
pub struct SonosDevice {
    url: Url,
    device: DeviceSpec,
}

impl SonosDevice {
    pub async fn from_ip(addr: Ipv4Addr) -> Result<Self> {
        Self::from_url(format!("http://{addr}:1400/xml/device_description.xml").parse()?).await
    }

    pub async fn from_url(url: Url) -> Result<Self> {
        let response = reqwest::get(url.clone()).await?;

        let status = response.status();
        if !status.is_success() {
            let body = match response.bytes().await {
                Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Err(err) => format!("Failed to retrieve body from failed request: {err:#}"),
            };

            return Err(Error::FailedRequest { status, body });
        }

        let body = response.text().await?;
        let device = DeviceSpec::parse_xml(&body)?;

        Ok(Self { url, device })
    }

    pub async fn name(&self) -> Result<String> {
        let attr = self.get_zone_attributes().await?;
        attr.current_zone_name.ok_or(Error::NoName)
    }

    pub async fn get_zone_group_state(&self) -> Result<Vec<ZoneGroup>> {
        let state = <Self as ZoneGroupTopology>::get_zone_group_state(self).await?;
        ZoneGroup::parse_xml(&state.zone_group_state.as_deref().unwrap_or(""))
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

impl SonosDevice {
    pub fn device_spec(&self) -> &DeviceSpec {
        &self.device
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
        RESP: FromXmlOwned + std::fmt::Debug,
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

        let status = response.status();
        if !status.is_success() {
            let body = match response.bytes().await {
                Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Err(err) => format!("Failed to retrieve body from failed request: {err:#}"),
            };

            return Err(Error::FailedRequest { status, body });
        }

        let body = response.text().await?;
        log::trace!("Got response: {body}");

        let envelope: soap_resp::Envelope<RESP> = instant_xml::from_str(&body)?;

        Ok(envelope.body.payload)
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
