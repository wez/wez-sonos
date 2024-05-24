use crate::upnp::DecodeXml;
use instant_xml::FromXml;

#[derive(Debug, PartialEq, Clone)]
pub struct ZoneGroupState {
    pub groups: Vec<ZoneGroup>,
}

impl DecodeXml for ZoneGroupState {
    fn decode_xml(xml: &str) -> crate::Result<Self> {
        let mut parsed: ZoneGroupStateHelper = instant_xml::from_str(xml)?;

        for group in &mut parsed.group_list.groups {
            group.members.sort_by(|a, b| a.uuid.cmp(&b.uuid));
        }

        Ok(Self {
            groups: parsed.group_list.groups,
        })
    }
}

#[derive(Debug, FromXml)]
#[xml(rename = "ZoneGroupState")]
struct ZoneGroupStateHelper {
    group_list: ZoneGroups,
    // There's a <VanishedDevices> element but I don't
    // know what it contains
}

#[derive(Debug, FromXml)]
struct ZoneGroups {
    pub groups: Vec<ZoneGroup>,
}

#[derive(Debug, FromXml, PartialEq, Eq, Clone)]
pub struct ZoneGroup {
    #[xml(rename = "Coordinator", attribute)]
    pub coordinator: String,
    #[xml(rename = "ID", attribute)]
    pub id: String,

    pub members: Vec<ZoneGroupMember>,
}

/// Helper for DRY; Satellite and ZoneGroupMember are almost
/// identical structs but have to be separate in order for
/// instant_xml to generate appropriate serde logic
macro_rules! machine_info {
    (pub struct $ty:ident { $($inner:tt)* }) => {
#[derive(Debug, FromXml, PartialEq, Eq, Clone)]
pub struct $ty {
    $($inner)*

    #[xml(rename = "UUID", attribute)]
    pub uuid: String,
    /// URL of the device_description.xml
    #[xml(rename = "Location", attribute)]
    pub location: String,
    #[xml(rename = "ZoneName", attribute)]
    pub zone_name: String,
    #[xml(rename = "Icon", attribute)]
    pub icon: String,
    #[xml(rename = "Configuration", attribute)]
    pub configuration: String,
    #[xml(rename = "SoftwareVersion", attribute)]
    pub software_version: String,
    #[xml(rename = "SWGen", attribute)]
    pub sw_gen: String,
    #[xml(rename = "MinCompatibleVersion", attribute)]
    pub min_compatible_version: String,
    #[xml(rename = "LegacyCompatibleVersion", attribute)]
    pub legacy_compatible_version: String,
    #[xml(rename = "BootSeq", attribute)]
    pub boot_seq: String,
    #[xml(rename = "TVConfigurationError", attribute)]
    pub tv_configuration_error: String,
    #[xml(rename = "HdmiCecAvailable", attribute)]
    pub hdmi_cec_available: u8,
    #[xml(rename = "WirelessMode", attribute)]
    pub wireless_mode: u8,
    #[xml(rename = "WirelessLeafOnly", attribute)]
    pub wireless_leaf_only: u8,
    #[xml(rename = "ChannelFreq", attribute)]
    pub channel_freq: u32,
    #[xml(rename = "BehindWifiExtender", attribute)]
    pub behind_wifi_extender: u8,
    #[xml(rename = "WifiEnabled", attribute)]
    pub wifi_enabled: u8,
    #[xml(rename = "EthLink", attribute)]
    pub eth_link: u8,
    #[xml(rename = "Orientation", attribute)]
    pub orientation: u8,
    #[xml(rename = "RoomCalibrationState", attribute)]
    pub room_calibration_state: u32,
    #[xml(rename = "SecureRegState", attribute)]
    pub secure_reg_state: u32,
    #[xml(rename = "VoiceConfigState", attribute)]
    pub voice_config_state: u32,
    #[xml(rename = "MicEnabled", attribute)]
    pub mic_enabled: u8,
    #[xml(rename = "AirPlayEnabled", attribute)]
    pub airplay_enabled: u8,
    #[xml(rename = "IdleState", attribute)]
    pub idle_state: u8,
    #[xml(rename = "MoreInfo", attribute)]
    pub more_info: String,
    #[xml(rename = "SSLPort", attribute)]
    pub ssl_port: u16,
    #[xml(rename = "HHSSLPort", attribute)]
    pub hhssl_port: u16,
}
    };
}

machine_info! {
    pub struct ZoneGroupMember {
        pub satellites: Vec<Satellite>,
    }
}

machine_info! {
    pub struct Satellite {
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_group_state() {
        let group_state = include_str!("../data/zone_group_state.xml");
        let parsed = ZoneGroupState::decode_xml(&group_state).unwrap();
        k9::snapshot!(
            parsed,
            r#"
ZoneGroupState {
    groups: [
        ZoneGroup {
            coordinator: "RINCON_XXX",
            id: "RINCON_XXX:3435548679",
            members: [
                ZoneGroupMember {
                    satellites: [],
                    uuid: "RINCON_XXX",
                    location: "http://10.10.10.161:1400/xml/device_description.xml",
                    zone_name: "Primary Bath",
                    icon: "",
                    configuration: "1",
                    software_version: "78.1-52020",
                    sw_gen: "2",
                    min_compatible_version: "77.0-00000",
                    legacy_compatible_version: "58.0-00000",
                    boot_seq: "145",
                    tv_configuration_error: "0",
                    hdmi_cec_available: 0,
                    wireless_mode: 1,
                    wireless_leaf_only: 0,
                    channel_freq: 5220,
                    behind_wifi_extender: 0,
                    wifi_enabled: 1,
                    eth_link: 0,
                    orientation: 0,
                    room_calibration_state: 4,
                    secure_reg_state: 3,
                    voice_config_state: 0,
                    mic_enabled: 0,
                    airplay_enabled: 1,
                    idle_state: 1,
                    more_info: "RawBattPct:99,BattPct:100,BattChg:CHARGING,BattTmp:33",
                    ssl_port: 1443,
                    hhssl_port: 1843,
                },
            ],
        },
        ZoneGroup {
            coordinator: "RINCON_XXX",
            id: "RINCON_XXX:3326086195",
            members: [
                ZoneGroupMember {
                    satellites: [
                        Satellite {
                            uuid: "RINCON_XXX",
                            location: "http://10.10.10.131:1400/xml/device_description.xml",
                            zone_name: "Some Room",
                            icon: "",
                            configuration: "1",
                            software_version: "78.1-52020",
                            sw_gen: "2",
                            min_compatible_version: "77.0-00000",
                            legacy_compatible_version: "58.0-00000",
                            boot_seq: "237",
                            tv_configuration_error: "0",
                            hdmi_cec_available: 0,
                            wireless_mode: 0,
                            wireless_leaf_only: 0,
                            channel_freq: 2437,
                            behind_wifi_extender: 0,
                            wifi_enabled: 1,
                            eth_link: 0,
                            orientation: 0,
                            room_calibration_state: 5,
                            secure_reg_state: 3,
                            voice_config_state: 0,
                            mic_enabled: 0,
                            airplay_enabled: 0,
                            idle_state: 1,
                            more_info: "",
                            ssl_port: 1443,
                            hhssl_port: 1843,
                        },
                        Satellite {
                            uuid: "RINCON_XXX",
                            location: "http://10.10.10.226:1400/xml/device_description.xml",
                            zone_name: "Some Room",
                            icon: "",
                            configuration: "1",
                            software_version: "78.1-52020",
                            sw_gen: "2",
                            min_compatible_version: "77.0-00000",
                            legacy_compatible_version: "58.0-00000",
                            boot_seq: "274",
                            tv_configuration_error: "0",
                            hdmi_cec_available: 0,
                            wireless_mode: 0,
                            wireless_leaf_only: 0,
                            channel_freq: 2437,
                            behind_wifi_extender: 0,
                            wifi_enabled: 1,
                            eth_link: 0,
                            orientation: 0,
                            room_calibration_state: 5,
                            secure_reg_state: 3,
                            voice_config_state: 0,
                            mic_enabled: 0,
                            airplay_enabled: 0,
                            idle_state: 1,
                            more_info: "",
                            ssl_port: 1443,
                            hhssl_port: 1843,
                        },
                    ],
                    uuid: "RINCON_XXX",
                    location: "http://10.10.10.196:1400/xml/device_description.xml",
                    zone_name: "Some Room",
                    icon: "",
                    configuration: "1",
                    software_version: "78.1-52020",
                    sw_gen: "2",
                    min_compatible_version: "77.0-00000",
                    legacy_compatible_version: "58.0-00000",
                    boot_seq: "123",
                    tv_configuration_error: "0",
                    hdmi_cec_available: 1,
                    wireless_mode: 0,
                    wireless_leaf_only: 0,
                    channel_freq: 2437,
                    behind_wifi_extender: 0,
                    wifi_enabled: 1,
                    eth_link: 0,
                    orientation: 0,
                    room_calibration_state: 1,
                    secure_reg_state: 3,
                    voice_config_state: 0,
                    mic_enabled: 0,
                    airplay_enabled: 1,
                    idle_state: 1,
                    more_info: "",
                    ssl_port: 1443,
                    hhssl_port: 1843,
                },
            ],
        },
        ZoneGroup {
            coordinator: "RINCON_XXX",
            id: "RINCON_XXX:2302873263",
            members: [
                ZoneGroupMember {
                    satellites: [],
                    uuid: "RINCON_XXX",
                    location: "http://10.10.10.166:1400/xml/device_description.xml",
                    zone_name: "Study",
                    icon: "",
                    configuration: "1",
                    software_version: "78.1-52020",
                    sw_gen: "2",
                    min_compatible_version: "77.0-00000",
                    legacy_compatible_version: "58.0-00000",
                    boot_seq: "73",
                    tv_configuration_error: "0",
                    hdmi_cec_available: 0,
                    wireless_mode: 0,
                    wireless_leaf_only: 0,
                    channel_freq: 2437,
                    behind_wifi_extender: 0,
                    wifi_enabled: 1,
                    eth_link: 1,
                    orientation: 0,
                    room_calibration_state: 4,
                    secure_reg_state: 3,
                    voice_config_state: 0,
                    mic_enabled: 0,
                    airplay_enabled: 1,
                    idle_state: 0,
                    more_info: "TargetRoomName:Study",
                    ssl_port: 1443,
                    hhssl_port: 1843,
                },
            ],
        },
        ZoneGroup {
            coordinator: "RINCON_XXX",
            id: "RINCON_XXX:4111376911",
            members: [
                ZoneGroupMember {
                    satellites: [],
                    uuid: "RINCON_XXX",
                    location: "http://10.10.10.138:1400/xml/device_description.xml",
                    zone_name: "Beam",
                    icon: "x-rincon-roomicon:masterbedroom",
                    configuration: "1",
                    software_version: "78.1-52020",
                    sw_gen: "2",
                    min_compatible_version: "77.0-00000",
                    legacy_compatible_version: "58.0-00000",
                    boot_seq: "158",
                    tv_configuration_error: "0",
                    hdmi_cec_available: 1,
                    wireless_mode: 0,
                    wireless_leaf_only: 0,
                    channel_freq: 2437,
                    behind_wifi_extender: 0,
                    wifi_enabled: 1,
                    eth_link: 0,
                    orientation: 0,
                    room_calibration_state: 3,
                    secure_reg_state: 3,
                    voice_config_state: 0,
                    mic_enabled: 0,
                    airplay_enabled: 1,
                    idle_state: 1,
                    more_info: "",
                    ssl_port: 1443,
                    hhssl_port: 1843,
                },
            ],
        },
        ZoneGroup {
            coordinator: "RINCON_XXX",
            id: "RINCON_XXX:2134456247",
            members: [
                ZoneGroupMember {
                    satellites: [],
                    uuid: "RINCON_XXX",
                    location: "http://10.10.10.165:1400/xml/device_description.xml",
                    zone_name: "Kitchen (Move)",
                    icon: "",
                    configuration: "1",
                    software_version: "78.1-52020",
                    sw_gen: "2",
                    min_compatible_version: "77.0-00000",
                    legacy_compatible_version: "58.0-00000",
                    boot_seq: "112",
                    tv_configuration_error: "0",
                    hdmi_cec_available: 0,
                    wireless_mode: 1,
                    wireless_leaf_only: 0,
                    channel_freq: 5785,
                    behind_wifi_extender: 0,
                    wifi_enabled: 1,
                    eth_link: 0,
                    orientation: 0,
                    room_calibration_state: 4,
                    secure_reg_state: 3,
                    voice_config_state: 0,
                    mic_enabled: 0,
                    airplay_enabled: 1,
                    idle_state: 1,
                    more_info: "RawBattPct:100,BattPct:100,BattChg:CHARGING,BattTmp:27",
                    ssl_port: 1443,
                    hhssl_port: 1843,
                },
            ],
        },
        ZoneGroup {
            coordinator: "RINCON_XXX",
            id: "RINCON_XXX:2884078592",
            members: [
                ZoneGroupMember {
                    satellites: [
                        Satellite {
                            uuid: "RINCON_XXX",
                            location: "http://10.10.10.190:1400/xml/device_description.xml",
                            zone_name: "Primary Bedroom",
                            icon: "x-rincon-roomicon:masterbedroom",
                            configuration: "1",
                            software_version: "78.1-52020",
                            sw_gen: "2",
                            min_compatible_version: "77.0-00000",
                            legacy_compatible_version: "58.0-00000",
                            boot_seq: "286",
                            tv_configuration_error: "0",
                            hdmi_cec_available: 0,
                            wireless_mode: 0,
                            wireless_leaf_only: 0,
                            channel_freq: 2437,
                            behind_wifi_extender: 0,
                            wifi_enabled: 1,
                            eth_link: 0,
                            orientation: 0,
                            room_calibration_state: 5,
                            secure_reg_state: 3,
                            voice_config_state: 0,
                            mic_enabled: 0,
                            airplay_enabled: 0,
                            idle_state: 1,
                            more_info: "",
                            ssl_port: 1443,
                            hhssl_port: 1843,
                        },
                        Satellite {
                            uuid: "RINCON_XXX",
                            location: "http://10.10.10.198:1400/xml/device_description.xml",
                            zone_name: "Primary Bedroom",
                            icon: "x-rincon-roomicon:masterbedroom",
                            configuration: "1",
                            software_version: "78.1-52020",
                            sw_gen: "2",
                            min_compatible_version: "77.0-00000",
                            legacy_compatible_version: "58.0-00000",
                            boot_seq: "278",
                            tv_configuration_error: "0",
                            hdmi_cec_available: 0,
                            wireless_mode: 0,
                            wireless_leaf_only: 0,
                            channel_freq: 2437,
                            behind_wifi_extender: 0,
                            wifi_enabled: 1,
                            eth_link: 0,
                            orientation: 0,
                            room_calibration_state: 5,
                            secure_reg_state: 3,
                            voice_config_state: 0,
                            mic_enabled: 0,
                            airplay_enabled: 0,
                            idle_state: 1,
                            more_info: "",
                            ssl_port: 1443,
                            hhssl_port: 1843,
                        },
                        Satellite {
                            uuid: "RINCON_XXX",
                            location: "http://10.10.10.116:1400/xml/device_description.xml",
                            zone_name: "Sub",
                            icon: "",
                            configuration: "1",
                            software_version: "78.1-52020",
                            sw_gen: "2",
                            min_compatible_version: "77.0-00000",
                            legacy_compatible_version: "58.0-00000",
                            boot_seq: "90",
                            tv_configuration_error: "0",
                            hdmi_cec_available: 0,
                            wireless_mode: 0,
                            wireless_leaf_only: 0,
                            channel_freq: 2437,
                            behind_wifi_extender: 0,
                            wifi_enabled: 1,
                            eth_link: 0,
                            orientation: 0,
                            room_calibration_state: 5,
                            secure_reg_state: 3,
                            voice_config_state: 0,
                            mic_enabled: 0,
                            airplay_enabled: 0,
                            idle_state: 1,
                            more_info: "",
                            ssl_port: 1443,
                            hhssl_port: 1843,
                        },
                    ],
                    uuid: "RINCON_XXX",
                    location: "http://10.10.10.231:1400/xml/device_description.xml",
                    zone_name: "Primary Bedroom",
                    icon: "",
                    configuration: "1",
                    software_version: "78.1-52020",
                    sw_gen: "2",
                    min_compatible_version: "77.0-00000",
                    legacy_compatible_version: "58.0-00000",
                    boot_seq: "91",
                    tv_configuration_error: "0",
                    hdmi_cec_available: 1,
                    wireless_mode: 0,
                    wireless_leaf_only: 0,
                    channel_freq: 2437,
                    behind_wifi_extender: 0,
                    wifi_enabled: 1,
                    eth_link: 0,
                    orientation: 0,
                    room_calibration_state: 1,
                    secure_reg_state: 3,
                    voice_config_state: 0,
                    mic_enabled: 0,
                    airplay_enabled: 1,
                    idle_state: 1,
                    more_info: "",
                    ssl_port: 1443,
                    hhssl_port: 1843,
                },
            ],
        },
        ZoneGroup {
            coordinator: "RINCON_XXX",
            id: "RINCON_XXX:1940091512",
            members: [
                ZoneGroupMember {
                    satellites: [],
                    uuid: "RINCON_XXX",
                    location: "http://10.10.10.157:1400/xml/device_description.xml",
                    zone_name: "Great Room",
                    icon: "",
                    configuration: "1",
                    software_version: "78.1-52020",
                    sw_gen: "2",
                    min_compatible_version: "77.0-00000",
                    legacy_compatible_version: "58.0-00000",
                    boot_seq: "89",
                    tv_configuration_error: "0",
                    hdmi_cec_available: 0,
                    wireless_mode: 0,
                    wireless_leaf_only: 0,
                    channel_freq: 2437,
                    behind_wifi_extender: 0,
                    wifi_enabled: 1,
                    eth_link: 1,
                    orientation: 0,
                    room_calibration_state: 4,
                    secure_reg_state: 3,
                    voice_config_state: 0,
                    mic_enabled: 0,
                    airplay_enabled: 1,
                    idle_state: 1,
                    more_info: "",
                    ssl_port: 1443,
                    hhssl_port: 1843,
                },
            ],
        },
        ZoneGroup {
            coordinator: "RINCON_XXX",
            id: "RINCON_XXX:2667033389",
            members: [
                ZoneGroupMember {
                    satellites: [],
                    uuid: "RINCON_XXX",
                    location: "http://10.10.10.120:1400/xml/device_description.xml",
                    zone_name: "Other Room",
                    icon: "x-rincon-roomicon:living",
                    configuration: "1",
                    software_version: "78.1-52020",
                    sw_gen: "2",
                    min_compatible_version: "77.0-00000",
                    legacy_compatible_version: "58.0-00000",
                    boot_seq: "320",
                    tv_configuration_error: "0",
                    hdmi_cec_available: 0,
                    wireless_mode: 0,
                    wireless_leaf_only: 0,
                    channel_freq: 2437,
                    behind_wifi_extender: 0,
                    wifi_enabled: 1,
                    eth_link: 0,
                    orientation: 0,
                    room_calibration_state: 5,
                    secure_reg_state: 3,
                    voice_config_state: 0,
                    mic_enabled: 0,
                    airplay_enabled: 0,
                    idle_state: 1,
                    more_info: "",
                    ssl_port: 1443,
                    hhssl_port: 1843,
                },
                ZoneGroupMember {
                    satellites: [],
                    uuid: "RINCON_XXX",
                    location: "http://10.10.10.158:1400/xml/device_description.xml",
                    zone_name: "Other Room",
                    icon: "x-rincon-roomicon:living",
                    configuration: "1",
                    software_version: "78.1-52020",
                    sw_gen: "2",
                    min_compatible_version: "77.0-00000",
                    legacy_compatible_version: "58.0-00000",
                    boot_seq: "273",
                    tv_configuration_error: "0",
                    hdmi_cec_available: 0,
                    wireless_mode: 0,
                    wireless_leaf_only: 0,
                    channel_freq: 2437,
                    behind_wifi_extender: 0,
                    wifi_enabled: 1,
                    eth_link: 0,
                    orientation: 4,
                    room_calibration_state: 3,
                    secure_reg_state: 3,
                    voice_config_state: 0,
                    mic_enabled: 0,
                    airplay_enabled: 1,
                    idle_state: 1,
                    more_info: "",
                    ssl_port: 1443,
                    hhssl_port: 1843,
                },
                ZoneGroupMember {
                    satellites: [],
                    uuid: "RINCON_XXX",
                    location: "http://10.10.10.217:1400/xml/device_description.xml",
                    zone_name: "Other Room",
                    icon: "x-rincon-roomicon:living",
                    configuration: "1",
                    software_version: "78.1-52020",
                    sw_gen: "2",
                    min_compatible_version: "77.0-00000",
                    legacy_compatible_version: "58.0-00000",
                    boot_seq: "253",
                    tv_configuration_error: "0",
                    hdmi_cec_available: 0,
                    wireless_mode: 0,
                    wireless_leaf_only: 0,
                    channel_freq: 2437,
                    behind_wifi_extender: 0,
                    wifi_enabled: 1,
                    eth_link: 0,
                    orientation: 3,
                    room_calibration_state: 5,
                    secure_reg_state: 3,
                    voice_config_state: 0,
                    mic_enabled: 0,
                    airplay_enabled: 0,
                    idle_state: 1,
                    more_info: "",
                    ssl_port: 1443,
                    hhssl_port: 1843,
                },
            ],
        },
        ZoneGroup {
            coordinator: "RINCON_XXX",
            id: "RINCON_XXX:97",
            members: [
                ZoneGroupMember {
                    satellites: [],
                    uuid: "RINCON_XXX",
                    location: "http://10.10.10.236:1400/xml/device_description.xml",
                    zone_name: "Kitchen",
                    icon: "x-rincon-roomicon:masterbedroom",
                    configuration: "1",
                    software_version: "78.1-52020",
                    sw_gen: "2",
                    min_compatible_version: "77.0-00000",
                    legacy_compatible_version: "58.0-00000",
                    boot_seq: "367",
                    tv_configuration_error: "0",
                    hdmi_cec_available: 0,
                    wireless_mode: 0,
                    wireless_leaf_only: 0,
                    channel_freq: 2437,
                    behind_wifi_extender: 0,
                    wifi_enabled: 1,
                    eth_link: 0,
                    orientation: 3,
                    room_calibration_state: 4,
                    secure_reg_state: 3,
                    voice_config_state: 0,
                    mic_enabled: 0,
                    airplay_enabled: 0,
                    idle_state: 1,
                    more_info: "",
                    ssl_port: 1443,
                    hhssl_port: 1843,
                },
            ],
        },
    ],
}
"#
        );
    }
}
