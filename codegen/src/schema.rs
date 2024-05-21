use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelInfo {
    pub model: String,
    pub model_description: String,
    pub software_generation: u32,
    pub software_version: String,
    pub discovery_date: DateTime<Utc>,
    pub services: Vec<ServiceInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceInfo {
    pub name: String,
    pub service_name: String,
    pub discovery_uri: String,
    pub service_id: String,
    pub service_type: String,
    #[serde(rename = "controlURL")]
    pub control_url: String,
    #[serde(rename = "eventSubURL")]
    pub event_sub_url: String,
    pub state_variables: Vec<StateVariable>,
    pub actions: Vec<Action>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateVariable {
    pub name: String,
    pub data_type: String,
    pub send_events: bool,
    #[serde(default)]
    pub allowed_values: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    pub name: String,
    #[serde(default)]
    pub inputs: Vec<Parameter>,
    #[serde(default)]
    pub outputs: Vec<Parameter>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    pub direction: String,
    pub related_state_variable_name: String,
}
