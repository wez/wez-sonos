use crate::{DecodeXml, EncodeXml, Error, Result};
use instant_xml::{FromXml, ToXml};
use std::time::Duration;

const XMLNS_DIDL_LITE: &str = "urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/";
const XMLNS_DC_ELEMENTS: &str = "http://purl.org/dc/elements/1.1/";
const XMLNS_UPNP: &str = "urn:schemas-upnp-org:metadata-1-0/upnp/";
const XMLNS_RINCONN: &str = "urn:schemas-rinconnetworks-com:metadata-1-0/";

/// Represents DIDL-Lite information but in a more ergonomic form.
/// This type can be converted to/from the corresponding DIDL-Lite
/// xml form.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TrackMetaData {
    pub title: String,
    pub creator: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
    pub url: String,
    pub mime_type: Option<String>,
    pub art_url: Option<String>,
    pub class: ObjectClass,
}

impl DecodeXml for TrackMetaData {
    fn decode_xml(xml: &str) -> Result<Self> {
        let mut list = Self::from_didl_str(xml)?;
        if list.len() == 1 {
            Ok(list.pop().expect("have 1"))
        } else if list.is_empty() {
            Err(Error::EmptyTrackMetaData)
        } else {
            Err(Error::MoreThanOneTrackMetaData)
        }
    }
}

impl EncodeXml for TrackMetaData {
    fn encode_xml(&self) -> std::result::Result<String, instant_xml::Error> {
        Ok(self.to_didl_string())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TrackMetaDataList {
    pub tracks: Vec<TrackMetaData>,
}

impl DecodeXml for TrackMetaDataList {
    fn decode_xml(xml: &str) -> Result<Self> {
        let tracks = TrackMetaData::from_didl_str(xml)?;
        Ok(Self { tracks })
    }
}

impl EncodeXml for TrackMetaDataList {
    fn encode_xml(&self) -> std::result::Result<String, instant_xml::Error> {
        let tracks: Vec<_> = self.tracks.iter().map(|t| t.to_didl_string()).collect();
        Ok(tracks.join(""))
    }
}

const HMS_FACTORS: &[u64] = &[86400, 3600, 60, 1];

pub fn duration_to_hms(d: Duration) -> String {
    use std::fmt::Write;
    let mut seconds_total = d.as_secs();
    let mut result = String::new();

    for &factor in HMS_FACTORS {
        let v = seconds_total / factor;
        seconds_total -= v * factor;

        if factor > 3600 && v == 0 {
            continue;
        }
        if !result.is_empty() {
            result.push(':');
        }
        if factor > 3600 {
            write!(&mut result, "{v}").ok();
        } else {
            write!(&mut result, "{v:02}").ok();
        }
    }

    result
}

pub fn hms_to_duration(hms: &str) -> Duration {
    let mut result = Duration::ZERO;

    for (field, factor) in hms.split(':').rev().zip(HMS_FACTORS.iter().rev()) {
        let Ok(v) = field.parse::<u64>() else {
            return Duration::ZERO;
        };
        result += Duration::from_secs(v * factor);
    }

    result
}

impl TrackMetaData {
    pub fn to_didl_string(&self) -> String {
        let didl = DidlLite {
            item: vec![UpnpItem {
                queue_item_id: None,
                mime_type: self
                    .mime_type
                    .clone()
                    .map(|mime_type| MimeType { mime_type }),
                duration: None,
                id: "-1".to_string(),
                parent_id: "-1".to_string(),
                restricted: true,
                res: Some(Res {
                    // Note that this assumes that the URL is an HTTP URL
                    protocol_info: format!(
                        "http-get:*:{}",
                        self.mime_type.as_deref().unwrap_or("audio/mpeg")
                    ),
                    duration: self
                        .duration
                        .map(duration_to_hms)
                        .unwrap_or_else(String::new),
                    url: self.url.to_string(),
                }),
                title: Some(Title {
                    title: self.title.to_string(),
                }),
                album_art: self.art_url.clone().map(|uri| AlbumArtUri { uri }),
                album_title: self
                    .album
                    .clone()
                    .map(|album_title| AlbumTitle { album_title }),
                creator: self.creator.clone().map(|artist| Creator { artist }),
                artist: self.creator.clone().map(|artist| Artist { artist }),
                class: Some(ObjectClass::MusicTrack),
            }],
        };
        instant_xml::to_string(&didl).expect("infallible xml encode!?")
    }

    pub fn from_didl_str(didl: &str) -> Result<Vec<Self>> {
        let didl: DidlLite = instant_xml::from_str(didl)?;
        let mut result = vec![];
        for item in didl.item {
            result.push(Self {
                class: item.class.unwrap_or_default(),
                album: item.album_title.map(|a| a.album_title),
                creator: item.creator.map(|a| a.artist),
                art_url: item.album_art.map(|a| a.uri),
                title: item.title.map(|a| a.title).unwrap_or_else(String::new),
                duration: match item.duration {
                    Some(d) => Some(Duration::from_secs(d.duration)),
                    None => item.res.as_ref().map(|r| hms_to_duration(&r.duration)),
                },
                url: item
                    .res
                    .as_ref()
                    .map(|r| r.url.to_string())
                    .unwrap_or_else(String::new),
                mime_type: item.res.as_ref().and_then(|r| {
                    let fields: Vec<&str> = r.protocol_info.split(':').collect();
                    fields.get(2).map(|mime_type| mime_type.to_string())
                }),
            });
        }
        Ok(result)
    }
}

#[derive(Debug, FromXml, ToXml)]
#[xml(rename="DIDL-Lite", ns(XMLNS_DIDL_LITE, dc=XMLNS_DC_ELEMENTS, upnp=XMLNS_UPNP, r=XMLNS_RINCONN))]
pub struct DidlLite {
    pub item: Vec<UpnpItem>,
}

#[derive(Debug, FromXml, ToXml)]
#[xml(rename = "item", ns(XMLNS_DIDL_LITE))]
pub struct UpnpItem {
    #[xml(attribute)]
    pub id: String,
    #[xml(attribute, rename = "parentID")]
    pub parent_id: String,
    #[xml(attribute)]
    pub restricted: bool,

    pub res: Option<Res>,
    pub duration: Option<UpnpDuration>,
    pub album_art: Option<AlbumArtUri>,
    pub album_title: Option<AlbumTitle>,
    pub artist: Option<Artist>,
    pub creator: Option<Creator>,
    pub title: Option<Title>,
    pub class: Option<ObjectClass>,
    pub mime_type: Option<MimeType>,
    pub queue_item_id: Option<QueueItemId>,
}

#[derive(Debug, FromXml, ToXml)]
#[xml(rename = "res", ns(XMLNS_DIDL_LITE))]
pub struct Res {
    #[xml(attribute, rename = "protocolInfo")]
    pub protocol_info: String,
    #[xml(attribute)]
    pub duration: String,
    #[xml(direct)]
    pub url: String,
}

#[derive(Debug, FromXml, ToXml)]
#[xml(rename="mimeType", ns(XMLNS_UPNP, upnp=XMLNS_UPNP))]
pub struct MimeType {
    #[xml(direct)]
    pub mime_type: String,
}

#[derive(Debug, FromXml, ToXml)]
#[xml(rename="albumArtURI", ns(XMLNS_UPNP, upnp=XMLNS_UPNP))]
pub struct AlbumArtUri {
    #[xml(direct)]
    pub uri: String,
}

#[derive(Debug, FromXml, ToXml)]
#[xml(rename="album", ns(XMLNS_UPNP, upnp=XMLNS_UPNP))]
pub struct AlbumTitle {
    #[xml(direct)]
    pub album_title: String,
}

#[derive(Debug, FromXml, ToXml)]
#[xml(rename="artist", ns(XMLNS_UPNP, upnp=XMLNS_UPNP))]
pub struct Artist {
    #[xml(direct)]
    pub artist: String,
}

#[derive(Debug, FromXml, ToXml)]
#[xml(rename="duration", ns(XMLNS_UPNP, upnp=XMLNS_UPNP))]
pub struct UpnpDuration {
    #[xml(direct)]
    pub duration: u64,
}

#[derive(Debug, FromXml, ToXml)]
#[xml(rename="creator", ns(XMLNS_DC_ELEMENTS, dc=XMLNS_DC_ELEMENTS))]
pub struct Creator {
    #[xml(direct)]
    pub artist: String,
}

#[derive(Debug, FromXml, ToXml)]
#[xml(rename="title", ns(XMLNS_DC_ELEMENTS, dc=XMLNS_DC_ELEMENTS))]
pub struct Title {
    #[xml(direct)]
    pub title: String,
}

#[derive(Debug, FromXml, ToXml)]
#[xml(rename="queueItemId", ns(XMLNS_DC_ELEMENTS, dc=XMLNS_DC_ELEMENTS))]
pub struct QueueItemId {
    #[xml(direct)]
    pub id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, FromXml, ToXml)]
#[xml(rename="class", scalar, ns(XMLNS_UPNP, upnp=XMLNS_UPNP))]
pub enum ObjectClass {
    #[xml(rename = "object.item.audioItem.musicTrack")]
    #[default]
    MusicTrack,
    #[xml(rename = "object.item.audioItem.audioBroadcast")]
    AudioBroadcast,
    #[xml(rename = "object.container.playlistContainer")]
    PlayList,
    #[xml(rename = "object.container")]
    Container,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_didl() {
        let didl = DidlLite {
            item: vec![UpnpItem {
                queue_item_id: None,
                mime_type: None,
                album_art: Some(AlbumArtUri {
                    uri: "http://art".to_string(),
                }),
                album_title: Some(AlbumTitle {
                    album_title: "My Album".to_string(),
                }),
                artist: None,
                creator: Some(Creator {
                    artist: "Some Guy".to_string(),
                }),
                class: Some(ObjectClass::MusicTrack),
                id: "-1".to_string(),
                parent_id: "-1".to_string(),
                res: Some(Res {
                    protocol_info: "http-get:*:audio/mpeg".to_string(),
                    duration: "0:30:31".to_string(),
                    url: "http://track.mp3".to_string(),
                }),
                duration: None,
                restricted: true,
                title: Some(Title {
                    title: "Track Title".to_string(),
                }),
            }],
        };
        k9::snapshot!(
            instant_xml::to_string(&didl).unwrap(),
            r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:r="urn:schemas-rinconnetworks-com:metadata-1-0/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/"><item id="-1" parentID="-1" restricted="true"><res protocolInfo="http-get:*:audio/mpeg" duration="0:30:31">http://track.mp3</res><upnp:albumArtURI>http://art</upnp:albumArtURI><upnp:album>My Album</upnp:album><dc:creator>Some Guy</dc:creator><dc:title>Track Title</dc:title><upnp:class>object.item.audioItem.musicTrack</upnp:class></item></DIDL-Lite>"#
        );
    }

    #[test]
    fn test_real_didl() {
        let input = r#"<DIDL-Lite xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dlna="urn:schemas-dlna-org:metadata-1-0/"><item id="1" parentID="0" restricted="1"><dc:title>Late Nights and Sneaky Moms</dc:title><dc:creator>DJ Birchy</dc:creator><upnp:album>[Unknown Album]</upnp:album><upnp:artist>DJ Borchy</upnp:artist><upnp:duration>4364</upnp:duration><dc:queueItemId>http://192.168.1.214:8097/single/RINCON_XXX/51f8b02b9d3b4a88b97dd385ba2b572b.flac?ts=1716507641</dc:queueItemId><upnp:albumArtURI>http://192.168.1.214:8097/imageproxy?path=al-573b45a1bde2b333c07b41545898da44_59330182&amp;provider=opensubsonic--EcQ6qYKn&amp;size=0&amp;fmt=png</upnp:albumArtURI><upnp:class>object.item.audioItem.audioBroadcast</upnp:class><upnp:mimeType>audio/flac</upnp:mimeType><res duration="1:12:44.000" protocolInfo="http-get:*:audio/flac:DLNA.ORG_PN=FLAC;DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=0d500000000000000000000000000000">http://192.168.1.214:8097/single/RINCON_XXX/51f8b02b9d3b4a88b97dd385ba2b572b.flac?ts=1716507641</res></item></DIDL-Lite>"#;
        let didl: DidlLite = instant_xml::from_str(&input).unwrap();
        k9::snapshot!(
            didl,
            r#"
DidlLite {
    item: [
        UpnpItem {
            id: "1",
            parent_id: "0",
            restricted: true,
            res: Some(
                Res {
                    protocol_info: "http-get:*:audio/flac:DLNA.ORG_PN=FLAC;DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=0d500000000000000000000000000000",
                    duration: "1:12:44.000",
                    url: "http://192.168.1.214:8097/single/RINCON_XXX/51f8b02b9d3b4a88b97dd385ba2b572b.flac?ts=1716507641",
                },
            ),
            duration: Some(
                UpnpDuration {
                    duration: 4364,
                },
            ),
            album_art: Some(
                AlbumArtUri {
                    uri: "http://192.168.1.214:8097/imageproxy?path=al-573b45a1bde2b333c07b41545898da44_59330182&provider=opensubsonic--EcQ6qYKn&size=0&fmt=png",
                },
            ),
            album_title: Some(
                AlbumTitle {
                    album_title: "[Unknown Album]",
                },
            ),
            artist: Some(
                Artist {
                    artist: "DJ Borchy",
                },
            ),
            creator: Some(
                Creator {
                    artist: "DJ Birchy",
                },
            ),
            title: Some(
                Title {
                    title: "Late Nights and Sneaky Moms",
                },
            ),
            class: Some(
                AudioBroadcast,
            ),
            mime_type: Some(
                MimeType {
                    mime_type: "audio/flac",
                },
            ),
            queue_item_id: Some(
                QueueItemId {
                    id: "http://192.168.1.214:8097/single/RINCON_XXX/51f8b02b9d3b4a88b97dd385ba2b572b.flac?ts=1716507641",
                },
            ),
        },
    ],
}
"#
        );
    }

    #[test]
    fn test_empty_album_art() {
        let input = r#"<DIDL-Lite xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns:r="urn:schemas-rinconnetworks-com:metadata-1-0/" xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/"><item id="00080000A%3aTRACKS" parentID="-1" restricted="true"><dc:title>Tracks</dc:title><upnp:class>object.container</upnp:class><desc id="cdudn" nameSpace="urn:schemas-rinconnetworks-com:metadata-1-0/"></desc><upnp:albumArtURI></upnp:albumArtURI></item></DIDL-Lite>"#;

        let didl: DidlLite = instant_xml::from_str(&input).unwrap();
        k9::snapshot!(didl);
    }

    #[test]
    fn test_hms() {
        fn r(hms: &str, s: u64) {
            assert_eq!(hms_to_duration(hms), Duration::from_secs(s));
            assert_eq!(duration_to_hms(Duration::from_secs(s)), hms);
        }

        r("00:02:31", 151);
        r("01:00:31", 3631);
        r("3:01:00:31", 262831);
    }
}
