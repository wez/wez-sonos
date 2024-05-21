use crate::schema::ModelInfo;
use crate::schema::Parameter;
use crate::schema::ServiceInfo;
use crate::schema::StateVariable;
use inflector::Inflector;
use serde_json::Value;
use ssdp_client::URN;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Write;

mod schema;

#[derive(Debug)]
pub struct VersionedService {
    pub info: ServiceInfo,
    pub state_variables: BTreeMap<String, StateVariable>,
    pub actions: BTreeMap<String, VersionedAction>,
}

impl VersionedService {
    fn resolve_type_for_param(&self, param: &VersionedParameter, always_optional: bool) -> String {
        let target = match self
            .state_variables
            .get(&param.param.related_state_variable_name)
        {
            Some(sv) => match sv.data_type.as_str() {
                "string" => "String",
                "ui4" => "u32",
                "ui2" => "u16",
                "i4" => "i32",
                "i2" => "i16",
                "boolean" => "bool",
                dt => unimplemented!("unhandled type {dt}"),
            }
            .to_string(),
            None => "String".to_string(),
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

fn main() {
    let mut models = BTreeMap::new();

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

    for (service_name, service) in &services {
        let service_module = to_snake_case(service_name);
        println!("Service {service_name}");

        let service_urn: URN = service.info.service_type.parse().unwrap();

        writeln!(&mut traits, "#[async_trait::async_trait]").ok();
        writeln!(&mut traits, "pub trait {service_name}Trait {{").ok();
        writeln!(&mut impls, "#[async_trait::async_trait]").ok();
        writeln!(&mut impls, "impl {service_name}Trait for SonosDevice {{").ok();

        writeln!(
            &mut types,
            "pub mod {service_module} {{
use ssdp_client::URN;
use instant_xml::{{FromXml, ToXml}};
"
        )
        .ok();

        writeln!(
            &mut types,
            "pub const SERVICE_TYPE: &URN = \
                 &URN::service(\"{}\", \"{}\", {});",
            service_urn.domain_name(),
            service_urn.typ(),
            service_urn.version()
        )
        .ok();

        writeln!(
            &mut types,
            "pub const SERVICE_NS: &str = \"{}\";",
            service.info.service_type
        )
        .ok();

        for (action_name, action) in &service.actions {
            let method_name = to_snake_case(action_name);
            //            println!("{action:#?}");

            let request_type_name = if action.inputs.is_empty() {
                "()".to_string()
            } else {
                let request_type_name = format!("{method_name}_request").to_pascal_case();
                if !action.inputs.is_empty() {
                    writeln!(&mut types, "#[derive(ToXml, Debug, Clone, PartialEq)]").ok();
                    writeln!(
                        &mut types,
                        "#[xml(rename=\"{action_name}\", ns(SERVICE_NS))]",
                    )
                    .ok();
                    writeln!(&mut types, "pub struct {request_type_name} {{").ok();
                    for p in &action.inputs {
                        let field_name = to_snake_case(&p.param.name);
                        let field_type = service.resolve_type_for_param(&p, false);
                        writeln!(
                            &mut types,
                            "  #[xml(rename=\"{}\", ns(\"\"))]",
                            p.param.name
                        )
                        .ok();
                        writeln!(&mut types, "  pub {field_name}: {field_type},").ok();
                    }
                    writeln!(&mut types, "}}").ok();
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
                    "#[xml(rename=\"{action_name}Response\", ns(SERVICE_NS))]",
                )
                .ok();
                writeln!(&mut types, "pub struct {response_type_name} {{").ok();
                for p in &action.outputs {
                    let field_name = to_snake_case(&p.param.name);
                    let field_type = service.resolve_type_for_param(&p, true);
                    writeln!(
                        &mut types,
                        "  #[xml(rename=\"{}\", ns(\"\"))]",
                        p.param.name
                    )
                    .ok();
                    writeln!(&mut types, "  pub {field_name}: {field_type},").ok();
                }
                writeln!(&mut types, "}}").ok();
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
            writeln!(&mut impls, "}}").ok();
            writeln!(&mut impls).ok();
        }

        writeln!(&mut traits, "}}").ok();
        writeln!(&mut impls, "}}").ok();
        writeln!(&mut types, "}}").ok();

        /*
        for (name, _sv) in &service.state_variables {
            let field_name = to_snake_case(name);
            println!("  var {name} {field_name}");
        }
        */
    }

    std::fs::write(
        "../src/generated.rs",
        format!(
            "// This file was auto-generated by codegen! Do not edit!

use crate::SonosDevice;
use crate::Result;

{types}
{traits}
{impls}
"
        ),
    )
    .unwrap();
}

fn to_snake_case(s: &str) -> String {
    // Fixup some special cases
    let s = s.replace("URIs", "Uris");
    let result = s.to_snake_case();
    if result == "type" {
        "type_".to_string()
    } else {
        result
    }
}
