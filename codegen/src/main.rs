use crate::schema::{ModelInfo, Parameter, ServiceInfo, StateVariable};
use inflector::Inflector;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

mod schema;

#[derive(Debug)]
pub struct VersionedService {
    pub info: ServiceInfo,
    pub state_variables: BTreeMap<String, StateVariable>,
    pub actions: BTreeMap<String, VersionedAction>,
}

fn refine_name(name: &str) -> String {
    name.replace("A_ARG_TYPE_", "")
}

impl VersionedService {
    fn resolve_type_for_sv(
        &self,
        name: &str,
        field_name: &str,
        sv: &StateVariable,
        always_optional: bool,
        containing_struct_name: &str,
    ) -> String {
        let refined_name = refine_name(name);

        let target = if let Some(Value::Array(_)) = &sv.allowed_values {
            // Use an enum
            format!("super::{refined_name}")
        } else {
            if sv.data_type == "string" {
                let target = self.maybe_decode_xml(&refined_name, containing_struct_name);
                if target == "String" {
                    self.maybe_decode_xml(field_name, containing_struct_name)
                } else {
                    target
                }
            } else {
                match sv.data_type.as_str() {
                    "string" => "String",
                    "ui4" => "u32",
                    "ui2" => "u16",
                    "i4" => "i32",
                    "i2" => "i16",
                    "boolean" => "bool",
                    dt => unimplemented!("unhandled type {dt}"),
                }
                .to_string()
            }
        };
        if always_optional {
            format!("Option<{target}>")
        } else {
            target
        }
    }

    fn maybe_decode_xml(&self, name: &str, containing_struct_name: &str) -> String {
        enum Entry {
            Name(&'static str),
            Alias {
                name: &'static str,
                type_name: &'static str,
            },
            StructField {
                containing_struct_name: &'static str,
                name: &'static str,
                type_name: &'static str,
            },
        }

        impl Entry {
            fn matches(&self, field_name: &str, containing_struct: &str) -> bool {
                match self {
                    Self::Name(name) => field_name == *name,
                    Self::Alias { name, .. } => field_name == *name,
                    Self::StructField {
                        containing_struct_name,
                        name,
                        ..
                    } => containing_struct == *containing_struct_name && field_name == *name,
                }
            }

            fn type_name(&self) -> &'static str {
                match self {
                    Self::Name(type_name)
                    | Self::Alias { type_name, .. }
                    | Self::StructField { type_name, .. } => type_name,
                }
            }
        }

        const KNOWN_TYPES: &[Entry] = &[
            Entry::Name("ZoneGroupState"),
            Entry::Name("TrackMetaData"),
            Entry::Alias {
                name: "EnqueuedTransportURIMetaData",
                type_name: "TrackMetaData",
            },
            Entry::Alias {
                name: "AVTransportURIMetaData",
                type_name: "TrackMetaData",
            },
            Entry::Alias {
                name: "EnqueuedURIMetaData",
                type_name: "TrackMetaData",
            },
            Entry::Alias {
                name: "CurrentTrackMetaData",
                type_name: "TrackMetaData",
            },
            Entry::StructField {
                containing_struct_name: "BrowseResponse",
                name: "Result",
                type_name: "TrackMetaDataList",
            },
        ];

        if let Some(entry) = KNOWN_TYPES
            .iter()
            .find(|e| e.matches(name, containing_struct_name))
        {
            // Use a wrapped version of this type
            format!("DecodeXmlString<crate::{}>", entry.type_name())
        } else {
            "String".to_string()
        }
    }

    fn resolve_type_for_param(
        &self,
        param: &VersionedParameter,
        always_optional: bool,
        containing_struct_name: &str,
    ) -> String {
        let target = match self
            .state_variables
            .get(&param.param.related_state_variable_name)
        {
            Some(sv) => self.resolve_type_for_sv(
                &param.param.related_state_variable_name,
                &param.param.name,
                sv,
                false,
                containing_struct_name,
            ),
            None => self.maybe_decode_xml(&param.param.name, containing_struct_name),
        };

        if param.optional || always_optional {
            format!("Option<{target}>")
        } else {
            target
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct VersionedAction {
    pub name: String,
    pub inputs: Vec<VersionedParameter>,
    pub outputs: Vec<VersionedParameter>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct VersionedParameter {
    pub param: Parameter,
    pub supported_by: BTreeSet<String>,
    pub optional: bool,
}

fn make_supported_set(model: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    set.insert(model.to_string());
    set
}

fn apply_parameter(target: &mut Vec<VersionedParameter>, source: &[Parameter], model: &str) {
    let was_empty = target.is_empty();

    for (idx, source_param) in source.iter().enumerate() {
        match target.get_mut(idx) {
            Some(target_param) => {
                assert_eq!(
                    target_param.param, *source_param,
                    "index {idx} has conflicting parameters {target_param:?} vs {source:?}"
                );
                target_param.supported_by.insert(model.to_string());
            }
            None => {
                target.push(VersionedParameter {
                    param: source_param.clone(),
                    supported_by: make_supported_set(model),
                    optional: !was_empty,
                });
            }
        }
    }
}

fn merge_allowed_values(target: &mut Option<Value>, source: &Option<Value>) {
    match (target, source) {
        (Some(Value::Array(target)), Some(Value::Array(source))) => {
            for item in source.iter() {
                if target.iter().find(|i| *i == item).is_none() {
                    target.push(item.clone());
                }
            }
        }
        (Some(source), Some(target)) if source == target => {}
        (None, None) => {}
        stuff => unimplemented!("handle {stuff:?} case"),
    }
}

#[derive(Deserialize, Debug)]
struct Documentation {
    services: BTreeMap<String, ServiceDocs>,
}

#[derive(Deserialize, Debug)]
struct ServiceDocs {
    description: String,
    #[serde(default)]
    actions: BTreeMap<String, ActionDocs>,
}

#[derive(Deserialize, Debug)]
struct ActionDocs {
    description: String,
    #[serde(default)]
    params: BTreeMap<String, String>,
}

const R_NS: &str = "urn:schemas-rinconnetworks-com:metadata-1-0/";

struct LastMeta {
    service_name: &'static str,
    ns: &'static str,
    root_object: &'static str,
    extra_ns: &'static [(&'static str, &'static str)],
}

const LAST_NS: &[LastMeta] = &[
    // FIXME: there are some elements in an r: namespace
    LastMeta {
        service_name: "AVTransport",
        ns: "urn:schemas-upnp-org:metadata-1-0/AVT/",
        root_object: "InstanceID",
        extra_ns: &[
            ("NextTrackURI", R_NS),
            ("EnqueuedTransportURI", R_NS),
            ("EnqueuedTransportURIMetaData", R_NS),
            ("NextTrackMetaData", R_NS),
            ("CurrentValidPlayModes", R_NS),
            ("DirectControlClientID", R_NS),
            ("DirectControlIsSuspended", R_NS),
            ("DirectControlAccountID", R_NS),
            ("SleepTimerGeneration", R_NS),
            ("AlarmRunning", R_NS),
            ("SnoozeRunning", R_NS),
            ("RestartPending", R_NS),
        ],
    },
    LastMeta {
        service_name: "Queue",
        ns: "urn:schemas-sonos-com:metadata-1-0/Queue/",
        root_object: "QueueID",
        extra_ns: &[],
    },
    // FIXME: There are things like `<Volume channel="RF" val="100"/><Mute
    // channel="Master" val="0"/>` in the RCS data, that we cannot
    // handle here.
    LastMeta {
        service_name: "RenderingControl",
        ns: "urn:schemas-upnp-org:metadata-1-0/RCS/",
        root_object: "InstanceID",
        extra_ns: &[],
    },
];

fn main() {
    let mut models = BTreeMap::new();
    let docs: Documentation =
        serde_json::from_slice(&std::fs::read("data/documentation.json").unwrap()).unwrap();

    for entry in std::fs::read_dir("data/devices").unwrap() {
        let entry = entry.unwrap();
        let meta = entry.metadata().unwrap();
        if meta.is_file() {
            let text = std::fs::read(entry.path()).unwrap();
            let info: ModelInfo = serde_json::from_slice(&text).unwrap();
            models.insert(info.model.to_string(), info);
        }
    }

    let mut services = BTreeMap::new();

    for info in models.values() {
        for service in &info.services {
            let entry = services.entry(service.name.clone()).or_insert_with(|| {
                let mut info = service.clone();
                info.state_variables.clear();
                info.actions.clear();
                VersionedService {
                    info,
                    state_variables: BTreeMap::new(),
                    actions: BTreeMap::new(),
                }
            });

            for var in &service.state_variables {
                let var_entry = entry
                    .state_variables
                    .entry(var.name.clone())
                    .or_insert_with(|| var.clone());

                // Some models don't support events for this one,
                // so let's assume that we can try it if any models do;
                // it will be a runtime error if the model doesn't support it.
                var_entry.send_events = var_entry.send_events || var.send_events;
                merge_allowed_values(&mut var_entry.allowed_values, &var.allowed_values);
            }

            for action in &service.actions {
                let action_entry =
                    entry
                        .actions
                        .entry(action.name.clone())
                        .or_insert_with(|| VersionedAction {
                            name: action.name.clone(),
                            inputs: vec![],
                            outputs: vec![],
                        });
                apply_parameter(&mut action_entry.inputs, &action.inputs, &info.model);
                apply_parameter(&mut action_entry.outputs, &action.outputs, &info.model);
            }
        }
    }

    let mut traits = String::new();
    let mut types = String::new();
    let mut impls = String::new();
    let mut prelude = String::new();

    for (service_name, service) in &services {
        let service_module = to_snake_case(service_name);
        println!("Service {service_name}");

        let service_type = &service.info.service_type;

        writeln!(&mut traits, "#[allow(async_fn_in_trait)]").ok();

        if let Some(doc) = docs
            .services
            .get(&format!("{service_name}Service"))
            .map(|s| &s.description)
        {
            writeln!(&mut traits, "/// {doc}").ok();
        }
        writeln!(&mut traits, "pub trait {service_name} {{").ok();
        writeln!(&mut prelude, "pub use super::{service_name};").ok();
        writeln!(&mut impls, "impl {service_name} for SonosDevice {{").ok();

        writeln!(
            &mut types,
            "/// Request and Response types for the `{service_name}` service.
            pub mod {service_module} {{
use super::*;
"
        )
        .ok();

        writeln!(
            &mut types,
            "/// URN for the `{service_name}` service.
            /// `{service_type}`
            pub const SERVICE_TYPE: &str = \"{service_type}\";\n",
        )
        .ok();

        let mut event_fields = BTreeMap::new();
        for (name, sv) in &service.state_variables {
            if sv.send_events {
                event_fields.insert(name, sv);
            }
        }
        for (action_name, action) in &service.actions {
            let method_name = to_snake_case(action_name);
            //            println!("{action:#?}");

            let request_type_name = if action.inputs.is_empty() {
                "()".to_string()
            } else {
                let request_type_name = format!("{method_name}_request").to_pascal_case();
                if !action.inputs.is_empty() {
                    writeln!(
                        &mut types,
                        "#[derive(ToXml, Debug, Clone, PartialEq, Default)]
                        #[xml(rename=\"{action_name}\", ns(SERVICE_TYPE))]",
                    )
                    .ok();
                    writeln!(&mut types, "pub struct {request_type_name} {{").ok();
                    for p in &action.inputs {
                        let field_name = to_snake_case(&p.param.name);
                        let field_type =
                            service.resolve_type_for_param(&p, false, &request_type_name);

                        if let Some(doc) = docs
                            .services
                            .get(&format!("{service_name}Service"))
                            .and_then(|s| s.actions.get(action_name))
                            .and_then(|a| a.params.get(&p.param.name))
                        {
                            writeln!(&mut types, "/// {doc}").ok();
                        }

                        writeln!(
                            &mut types,
                            "  #[xml(rename=\"{}\", ns(\"\"))]",
                            p.param.name
                        )
                        .ok();
                        writeln!(&mut types, "  pub {field_name}: {field_type},").ok();
                    }
                    writeln!(&mut types, "}}\n").ok();
                }
                format!("{service_module}::{request_type_name}")
            };

            let response_type_name = if action.outputs.is_empty() {
                "()".to_string()
            } else {
                let response_type_name = format!("{method_name}_response").to_pascal_case();
                writeln!(&mut types, "#[derive(FromXml, Debug, Clone, PartialEq)]").ok();
                writeln!(
                    &mut types,
                    "#[xml(rename=\"{action_name}Response\", ns(SERVICE_TYPE))]",
                )
                .ok();
                writeln!(&mut types, "pub struct {response_type_name} {{").ok();
                for p in &action.outputs {
                    let field_name = to_snake_case(&p.param.name);
                    let field_type = service.resolve_type_for_param(&p, true, &response_type_name);
                    writeln!(
                        &mut types,
                        "  #[xml(rename=\"{}\", ns(\"\"))]",
                        p.param.name
                    )
                    .ok();
                    writeln!(&mut types, "  pub {field_name}: {field_type},").ok();
                }
                writeln!(&mut types, "}}\n").ok();
                writeln!(
                    &mut types,
                    "
impl crate::DecodeSoapResponse for {response_type_name} {{
    fn decode_soap_xml(xml: &str) -> crate::Result<Self> {{
        let envelope: crate::soap_resp::Envelope<Self> = instant_xml::from_str(xml)?;
        Ok(envelope.body.payload)
    }}
}}
"
                )
                .ok();
                format!("{service_module}::{response_type_name}")
            };

            let params = if !action.inputs.is_empty() {
                format!(", request: {request_type_name}")
            } else {
                "".to_string()
            };

            let encode_payload = if !action.inputs.is_empty() {
                format!("request")
            } else {
                "crate::soap::Unit{}".to_string()
            };

            if let Some(doc) = docs
                .services
                .get(&format!("{service_name}Service"))
                .and_then(|s| s.actions.get(action_name))
                .map(|a| &a.description)
            {
                writeln!(&mut traits, "/// {doc}").ok();
            }
            writeln!(
                &mut traits,
                "async fn {method_name}(&self{params}) -> Result<{response_type_name}>;"
            )
            .ok();
            writeln!(
                &mut impls,
                "async fn {method_name}(&self{params}) -> Result<{response_type_name}> {{"
            )
            .ok();
            writeln!(&mut impls, "  self.action(&{service_module}::SERVICE_TYPE, \"{action_name}\", {encode_payload}).await").ok();
            writeln!(&mut impls, "}}\n").ok();
            writeln!(&mut impls).ok();
        }

        writeln!(&mut traits, "}}\n").ok();
        writeln!(&mut impls, "}}\n").ok();

        if !event_fields.is_empty() {
            writeln!(
                &mut types,
                "
/// A parsed event produced by the `{service_name}` service.
/// Use `SonosDevice::subscribe_{service_module}()` to obtain an event
/// stream that produces these.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct {service_name}Event {{"
            )
            .ok();

            let last_change_meta = LAST_NS.iter().find(|m| m.service_name == service_name);

            for (name, sv) in &event_fields {
                let field_name = to_snake_case(name);

                let field_type = if name.as_str() == "LastChange" && last_change_meta.is_some() {
                    format!("Option<DecodeXmlString<{service_name}LastChangeMap>>")
                } else {
                    service.resolve_type_for_sv(
                        &name,
                        &name,
                        sv,
                        true,
                        &format!("{service_name}Event"),
                    )
                };

                writeln!(&mut types, "  pub {field_name}: {field_type},").ok();
            }
            writeln!(&mut types, "}}").ok();

            // Generate a helper for decoding the xml into the above
            // ergonomic form

            writeln!(
                &mut types,
                r#"
#[derive(FromXml, Debug, Clone, PartialEq)]
#[xml(rename="propertyset", ns(crate::upnp::UPNP_EVENT, e=crate::upnp::UPNP_EVENT))]
struct {service_name}PropertySet {{
    pub properties: Vec<{service_name}Property>,
}}

#[derive(FromXml, Debug, Clone, PartialEq)]
#[xml(rename="property", ns(crate::upnp::UPNP_EVENT, e=crate::upnp::UPNP_EVENT))]
struct {service_name}Property {{
"#
            )
            .ok();

            for (name, sv) in &event_fields {
                let field_name = to_snake_case(name);

                let field_type = if name.as_str() == "LastChange" && last_change_meta.is_some() {
                    format!("Option<DecodeXmlString<{service_name}LastChangeMap>>")
                } else {
                    service.resolve_type_for_sv(
                        &name,
                        &name,
                        sv,
                        true,
                        &format!("{service_name}Event"),
                    )
                };

                writeln!(&mut types, "  #[xml(rename=\"{name}\", ns(\"\"))]",).ok();
                writeln!(&mut types, "  pub {field_name}: {field_type},").ok();
            }
            writeln!(&mut types, "}}").ok();

            writeln!(
                &mut types,
                r#"
impl DecodeXml for {service_name}Event {{
    fn decode_xml(xml: &str) -> crate::Result<Self> {{
        let mut result = Self::default();
        let set: {service_name}PropertySet = instant_xml::from_str(xml)?;
        for prop in set.properties {{
"#
            )
            .ok();

            for (name, _sv) in &event_fields {
                let field_name = to_snake_case(name);
                writeln!(
                    &mut types,
                    r#"
                    if let Some(v) = prop.{field_name} {{
                        result.{field_name}.replace(v);
                    }}
                    "#
                )
                .ok();
            }

            writeln!(&mut types, r#"
        }}
        Ok(result)
    }}
}}

impl crate::SonosDevice {{
    /// Subscribe to events from the `{service_name}` service on this device
    pub async fn subscribe_{service_module}(&self) -> crate::Result<crate::upnp::EventStream<{service_name}Event>> {{
        self.subscribe_helper(&SERVICE_TYPE).await
    }}
}}
"#).ok();

            if let Some(last_change) = &last_change_meta {
                writeln!(
                    &mut types,
                    r#"
    #[derive(Debug, Clone, PartialEq, Default)]
    pub struct {service_name}LastChange {{
    "#
                )
                .ok();

                let last_change_ns = last_change.ns;
                let last_change_root = last_change.root_object;

                let mut instance_wrapper = String::new();
                let mut attributes = String::new();
                let mut decode_logic = String::new();
                writeln!(
                    &mut instance_wrapper,
                    r#"
const LAST_CHANGE_NS: &str = "{last_change_ns}";

#[derive(FromXml)]
#[xml(rename="{last_change_root}", ns(LAST_CHANGE_NS))]
struct {service_name}LastChangeRootObject {{
    #[xml(rename="val", attribute)]
    object_instance_id_: u32,
                "#
                )
                .ok();

                let mut names_done = BTreeSet::new();
                for (name, sv) in &service.state_variables {
                    if name == "LastChange" {
                        continue;
                    }
                    let name = refine_name(&name);
                    if names_done.contains(&name) {
                        continue;
                    }
                    names_done.insert(name.to_string());

                    let field_name = to_snake_case(&name);
                    let field_type = service.resolve_type_for_sv(
                        &name,
                        &name,
                        sv,
                        true,
                        &format!("{service_name}Event"),
                    );

                    writeln!(&mut types, "  pub {field_name}: {field_type},").ok();

                    writeln!(
                        &mut instance_wrapper,
                        "  {field_name}: Option<{service_name}LastChange{name}>,"
                    )
                    .ok();

                    let field_ns = last_change
                        .extra_ns
                        .iter()
                        .find(|(element, _ns)| *element == name)
                        .map(|(_element, ns)| format!("\"{ns}\""))
                        .unwrap_or_else(|| "LAST_CHANGE_NS".to_string());

                    writeln!(
                        &mut attributes,
                        r#"
#[derive(FromXml)]
#[xml(rename="{name}", ns({field_ns}))]
#[allow(non_camel_case_types)]
struct {service_name}LastChange{name} {{
    #[xml(attribute)]
    val: {field_type},
}}
                "#
                    )
                    .ok();

                    writeln!(
                        &mut decode_logic,
                        r#"
result.{field_name} = item.{field_name}.and_then(|v| v.val);
                    "#
                    )
                    .ok();
                }
                writeln!(&mut instance_wrapper, "}}").ok();

                writeln!(
                    &mut types,
                    r#"}}
{attributes}

{instance_wrapper}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct {service_name}LastChangeMap {{
    pub map: std::collections::BTreeMap<u32, {service_name}LastChange>,
}}

impl DecodeXml for {service_name}LastChangeMap {{
    fn decode_xml(xml: &str) -> crate::Result<Self> {{
        #[derive(FromXml)]
        #[xml(ns(LAST_CHANGE_NS, r="urn:schemas-rinconnetworks-com:metadata-1-0/"))]
        struct Event {{
            instance: Vec<{service_name}LastChangeRootObject>,
        }}

        let last_change: Event = instant_xml::from_str(xml)?;
        let mut map = std::collections::BTreeMap::new();

        for item in last_change.instance {{
            let mut result = {service_name}LastChange::default();

            {decode_logic}

            map.insert(item.object_instance_id_, result);
        }}

        Ok({service_name}LastChangeMap{{map}})
    }}
}}
"#
                )
                .ok();
            } // if has_last_change
        }

        // close `mod service_module`
        writeln!(&mut types, "}}\n").ok();

        for (name, sv) in &service.state_variables {
            if let Some(Value::Array(allowed)) = &sv.allowed_values {
                let enum_name = refine_name(name);

                writeln!(
                    &mut types,
                    "#[derive(PartialEq, Debug, Clone, Eq, Default)]"
                )
                .ok();
                writeln!(&mut types, "pub enum {enum_name} {{").ok();
                for (idx, item) in allowed.iter().enumerate() {
                    let variant = item.to_string().to_pascal_case();
                    if idx == 0 {
                        writeln!(&mut types, "  #[default]").ok();
                    }
                    writeln!(&mut types, "  {variant},").ok();
                }
                writeln!(
                    &mut types,
                    "
/// Allows passing a value that was not known at the
/// time that this crate was generated from the available
/// device descriptions"
                )
                .ok();
                writeln!(&mut types, "  Unspecified(String),").ok();
                writeln!(&mut types, "}}\n").ok();

                writeln!(&mut types, "impl ToString for {enum_name} {{").ok();
                writeln!(&mut types, "fn to_string(&self) -> String {{").ok();
                writeln!(&mut types, "match self {{").ok();

                for item in allowed {
                    let variant = item.to_string().to_pascal_case();
                    writeln!(
                        &mut types,
                        "  {enum_name}::{variant} => {item}.to_string(),"
                    )
                    .ok();
                }

                writeln!(
                    &mut types,
                    "  {enum_name}::Unspecified(s) => s.to_string(),"
                )
                .ok();
                writeln!(&mut types, "}}").ok();
                writeln!(&mut types, "}}\n").ok();
                writeln!(&mut types, "}}\n").ok();

                writeln!(&mut types, "impl FromStr for {enum_name} {{").ok();
                writeln!(&mut types, "type Err = crate::Error;").ok();
                writeln!(&mut types, "fn from_str(s: &str) -> Result<{enum_name}> {{").ok();
                writeln!(&mut types, "match s {{").ok();

                for item in allowed {
                    let variant = item.to_string().to_pascal_case();
                    writeln!(&mut types, "  {item} => Ok({enum_name}::{variant}),").ok();
                }
                writeln!(
                    &mut types,
                    "s => Ok({enum_name}::Unspecified(s.to_string())),"
                )
                .ok();

                writeln!(&mut types, "}}").ok();
                writeln!(&mut types, "}}\n").ok();
                writeln!(&mut types, "}}\n").ok();

                writeln!(
                    &mut types,
                    "impl instant_xml::ToXml for {enum_name} {{
fn serialize<W: std::fmt::Write + ?Sized>(
    &self,
    field: Option<instant_xml::Id<'_>>,
    serializer: &mut instant_xml::Serializer<W>,
    ) -> std::result::Result<(), instant_xml::Error> {{
    self.to_string().serialize(field, serializer)
}}

fn present(&self) -> bool {{
    true
}}
}}

impl<'xml> instant_xml::FromXml<'xml> for {enum_name} {{
    #[inline]
    fn matches(id: instant_xml::Id<'_>, field: Option<instant_xml::Id<'_>>) -> bool {{
        match field {{
            Some(field) => id == field,
            None => false,
        }}
    }}

    fn deserialize<'cx>(
        into: &mut Self::Accumulator,
        field: &'static str,
        deserializer: &mut instant_xml::Deserializer<'cx, 'xml>,
        ) -> std::result::Result<(), instant_xml::Error> {{
        if into.is_some() {{
            return Err(instant_xml::Error::DuplicateValue);
        }}

        match deserializer.take_str()? {{
            Some(value) => {{
                let parsed: {enum_name} = value.parse().map_err(|err| {{
                    instant_xml::Error::Other(format!(
                            \"invalid value for field {{field}}: {{value}}: {{err:#}}\"
                            ))
                }})?;
                *into = Some(parsed);
                Ok(())
            }}
            None => Err(instant_xml::Error::MissingValue(field)),
        }}
    }}

    type Accumulator = Option<{enum_name}>;
    const KIND: instant_xml::Kind = instant_xml::Kind::Scalar;
}}


"
                )
                .ok();
            }
        }
    }

    std::fs::write(
        "../src/generated.rs",
        format!(
            "// This file was auto-generated by codegen! Do not edit!

use std::str::FromStr;
use crate::SonosDevice;
use crate::Result;
use instant_xml::{{FromXml, ToXml}};
use crate::upnp::DecodeXml;
use crate::xmlutil::DecodeXmlString;

{types}
{traits}
{impls}

/// The prelude makes it convenient to use the methods of `SonosDevice`.
/// Intended usage is `use sonos::prelude::*;` and then you don't have
/// to worry about importing the individual service traits.
pub mod prelude {{
{prelude}
}}
"
        ),
    )
    .unwrap();
}

fn to_snake_case(s: &str) -> String {
    // Fixup some special cases
    let s = s
        .replace("URIs", "Uris")
        .replace("UUIDs", "Uuids")
        .replace("IDs", "Ids");
    let result = s.to_snake_case();
    if result == "type" {
        "type_".to_string()
    } else {
        result
    }
}
