use serde::{Deserialize, Serialize};
use crate::types::{
    TunnelConfig, SplitTunnelingSettings, AppSettings,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Request {
    #[serde(rename = "list_tunnels")]
    ListTunnels,

    #[serde(rename = "get_tunnel")]
    GetTunnel { name: String },

    #[serde(rename = "add_tunnel")]
    AddTunnel { config: TunnelConfig },

    #[serde(rename = "update_tunnel")]
    UpdateTunnel { config: TunnelConfig },

    #[serde(rename = "delete_tunnel")]
    DeleteTunnel { name: String },

    #[serde(rename = "connect_tunnel")]
    ConnectTunnel { name: String },

    #[serde(rename = "disconnect_tunnel")]
    DisconnectTunnel,

    #[serde(rename = "get_status")]
    GetStatus,

    #[serde(rename = "get_settings")]
    GetSettings,

    #[serde(rename = "update_settings")]
    UpdateSettings { settings: AppSettings },

    #[serde(rename = "set_split_tunneling")]
    SetSplitTunneling { settings: SplitTunnelingSettings },

    #[serde(rename = "generate_keypair")]
    GenerateKeypair,

    #[serde(rename = "list_processes")]
    ListProcesses,

    #[serde(rename = "import_tunnel")]
    ImportTunnel { name: String, config_text: String },

    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum Response {
    #[serde(rename = "ok")]
    Ok(serde_json::Value),

    #[serde(rename = "error")]
    Error(String),
}

impl Response {
    pub fn ok(data: impl Serialize) -> Self {
        Response::Ok(serde_json::to_value(data).unwrap_or_else(|_| serde_json::Value::Null))
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Response::Error(msg.into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcMessage {
    pub id: u64,
    #[serde(flatten)]
    pub payload: IpcPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcPayload {
    #[serde(rename = "request")]
    Request { request: Request },
    #[serde(rename = "response")]
    Response { response: Response },
}
