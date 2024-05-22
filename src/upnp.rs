use instant_xml::FromXml;
use instant_xml::Id;
use instant_xml::Kind;
use reqwest::Url;
use ssdp_client::URN;

const UPNP_DEVICE: &str = "urn:schemas-upnp-org:device-1-0";

#[derive(Debug, PartialEq, Eq, Clone)]
#[repr(transparent)]
pub struct XmlWrapped<T>(T)
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display;

impl<T> std::ops::Deref for XmlWrapped<T>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<'xml, T> FromXml<'xml> for XmlWrapped<T>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    #[inline]
    fn matches(id: Id<'_>, field: Option<Id<'_>>) -> bool {
        match field {
            Some(field) => id == field,
            None => false,
        }
    }

    fn deserialize<'cx>(
        into: &mut Self::Accumulator,
        field: &'static str,
        deserializer: &mut instant_xml::Deserializer<'cx, 'xml>,
    ) -> Result<(), instant_xml::Error> {
        if into.is_some() {
            return Err(instant_xml::Error::DuplicateValue);
        }

        match deserializer.take_str()? {
            Some(value) => {
                let parsed: T = value.parse().map_err(|err| {
                    instant_xml::Error::Other(format!(
                        "invalid URN for field {field}: {value}: {err:#}"
                    ))
                })?;
                *into = Some(XmlWrapped(parsed));
                Ok(())
            }
            None => Err(instant_xml::Error::MissingValue(field)),
        }
    }

    type Accumulator = Option<XmlWrapped<T>>;
    const KIND: Kind = Kind::Scalar;
}

#[derive(Debug, FromXml)]
#[xml(rename = "device", ns(UPNP_DEVICE))]
pub struct DeviceSpec {
    #[xml(rename = "friendlyName")]
    pub friendly_name: String,
    #[xml(rename = "deviceType")]
    pub device_type: XmlWrapped<URN>,
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

    pub fn get_service(&self, service_type: &URN) -> Option<&Service> {
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
    pub service_type: XmlWrapped<URN>,
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
    pub fn control_url(&self, url: &Url) -> Url {
        match url.join(&self.control_url) {
            Ok(url) => url,
            Err(err) => {
                log::error!("Cannot join {url} with {}: {err:#}", self.control_url);
                url.clone()
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
        device_type: XmlWrapped(
            Device(
                "schemas-upnp-org",
                "ZonePlayer",
                1,
            ),
        ),
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
                        service_type: XmlWrapped(
                            Service(
                                "schemas-upnp-org",
                                "AlarmClock",
                                1,
                            ),
                        ),
                        service_id: "urn:upnp-org:serviceId:AlarmClock",
                        control_url: "/AlarmClock/Control",
                        event_sub_url: "/AlarmClock/Event",
                        scpd_url: "/xml/AlarmClock1.xml",
                    },
                    Service {
                        service_type: XmlWrapped(
                            Service(
                                "schemas-upnp-org",
                                "MusicServices",
                                1,
                            ),
                        ),
                        service_id: "urn:upnp-org:serviceId:MusicServices",
                        control_url: "/MusicServices/Control",
                        event_sub_url: "/MusicServices/Event",
                        scpd_url: "/xml/MusicServices1.xml",
                    },
                    Service {
                        service_type: XmlWrapped(
                            Service(
                                "schemas-upnp-org",
                                "AudioIn",
                                1,
                            ),
                        ),
                        service_id: "urn:upnp-org:serviceId:AudioIn",
                        control_url: "/AudioIn/Control",
                        event_sub_url: "/AudioIn/Event",
                        scpd_url: "/xml/AudioIn1.xml",
                    },
                    Service {
                        service_type: XmlWrapped(
                            Service(
                                "schemas-upnp-org",
                                "DeviceProperties",
                                1,
                            ),
                        ),
                        service_id: "urn:upnp-org:serviceId:DeviceProperties",
                        control_url: "/DeviceProperties/Control",
                        event_sub_url: "/DeviceProperties/Event",
                        scpd_url: "/xml/DeviceProperties1.xml",
                    },
                    Service {
                        service_type: XmlWrapped(
                            Service(
                                "schemas-upnp-org",
                                "SystemProperties",
                                1,
                            ),
                        ),
                        service_id: "urn:upnp-org:serviceId:SystemProperties",
                        control_url: "/SystemProperties/Control",
                        event_sub_url: "/SystemProperties/Event",
                        scpd_url: "/xml/SystemProperties1.xml",
                    },
                    Service {
                        service_type: XmlWrapped(
                            Service(
                                "schemas-upnp-org",
                                "ZoneGroupTopology",
                                1,
                            ),
                        ),
                        service_id: "urn:upnp-org:serviceId:ZoneGroupTopology",
                        control_url: "/ZoneGroupTopology/Control",
                        event_sub_url: "/ZoneGroupTopology/Event",
                        scpd_url: "/xml/ZoneGroupTopology1.xml",
                    },
                    Service {
                        service_type: XmlWrapped(
                            Service(
                                "schemas-upnp-org",
                                "GroupManagement",
                                1,
                            ),
                        ),
                        service_id: "urn:upnp-org:serviceId:GroupManagement",
                        control_url: "/GroupManagement/Control",
                        event_sub_url: "/GroupManagement/Event",
                        scpd_url: "/xml/GroupManagement1.xml",
                    },
                    Service {
                        service_type: XmlWrapped(
                            Service(
                                "schemas-tencent-com",
                                "QPlay",
                                1,
                            ),
                        ),
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
                        device_type: XmlWrapped(
                            Device(
                                "schemas-upnp-org",
                                "MediaServer",
                                1,
                            ),
                        ),
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
                                        service_type: XmlWrapped(
                                            Service(
                                                "schemas-upnp-org",
                                                "ContentDirectory",
                                                1,
                                            ),
                                        ),
                                        service_id: "urn:upnp-org:serviceId:ContentDirectory",
                                        control_url: "/MediaServer/ContentDirectory/Control",
                                        event_sub_url: "/MediaServer/ContentDirectory/Event",
                                        scpd_url: "/xml/ContentDirectory1.xml",
                                    },
                                    Service {
                                        service_type: XmlWrapped(
                                            Service(
                                                "schemas-upnp-org",
                                                "ConnectionManager",
                                                1,
                                            ),
                                        ),
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
                        device_type: XmlWrapped(
                            Device(
                                "schemas-upnp-org",
                                "MediaRenderer",
                                1,
                            ),
                        ),
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
                                        service_type: XmlWrapped(
                                            Service(
                                                "schemas-upnp-org",
                                                "RenderingControl",
                                                1,
                                            ),
                                        ),
                                        service_id: "urn:upnp-org:serviceId:RenderingControl",
                                        control_url: "/MediaRenderer/RenderingControl/Control",
                                        event_sub_url: "/MediaRenderer/RenderingControl/Event",
                                        scpd_url: "/xml/RenderingControl1.xml",
                                    },
                                    Service {
                                        service_type: XmlWrapped(
                                            Service(
                                                "schemas-upnp-org",
                                                "ConnectionManager",
                                                1,
                                            ),
                                        ),
                                        service_id: "urn:upnp-org:serviceId:ConnectionManager",
                                        control_url: "/MediaRenderer/ConnectionManager/Control",
                                        event_sub_url: "/MediaRenderer/ConnectionManager/Event",
                                        scpd_url: "/xml/ConnectionManager1.xml",
                                    },
                                    Service {
                                        service_type: XmlWrapped(
                                            Service(
                                                "schemas-upnp-org",
                                                "AVTransport",
                                                1,
                                            ),
                                        ),
                                        service_id: "urn:upnp-org:serviceId:AVTransport",
                                        control_url: "/MediaRenderer/AVTransport/Control",
                                        event_sub_url: "/MediaRenderer/AVTransport/Event",
                                        scpd_url: "/xml/AVTransport1.xml",
                                    },
                                    Service {
                                        service_type: XmlWrapped(
                                            Service(
                                                "schemas-sonos-com",
                                                "Queue",
                                                1,
                                            ),
                                        ),
                                        service_id: "urn:sonos-com:serviceId:Queue",
                                        control_url: "/MediaRenderer/Queue/Control",
                                        event_sub_url: "/MediaRenderer/Queue/Event",
                                        scpd_url: "/xml/Queue1.xml",
                                    },
                                    Service {
                                        service_type: XmlWrapped(
                                            Service(
                                                "schemas-upnp-org",
                                                "GroupRenderingControl",
                                                1,
                                            ),
                                        ),
                                        service_id: "urn:upnp-org:serviceId:GroupRenderingControl",
                                        control_url: "/MediaRenderer/GroupRenderingControl/Control",
                                        event_sub_url: "/MediaRenderer/GroupRenderingControl/Event",
                                        scpd_url: "/xml/GroupRenderingControl1.xml",
                                    },
                                    Service {
                                        service_type: XmlWrapped(
                                            Service(
                                                "schemas-upnp-org",
                                                "VirtualLineIn",
                                                1,
                                            ),
                                        ),
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
