use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;
use log::{info, debug, error, warn};
use serde_json::json;
use wgsplit_common::proto::{Request, Response, IpcMessage, IpcPayload};
use wgsplit_common::types::TunnelStatus;

use crate::AppState;

pub struct IpcServer {
    state: Arc<AppState>,
    socket_path: String,
}

impl IpcServer {
    pub fn new(state: Arc<AppState>, socket_path: &str) -> anyhow::Result<Self> {
        if std::path::Path::new(socket_path).exists() {
            std::fs::remove_file(socket_path)?;
        }
        Ok(Self {
            state,
            socket_path: socket_path.to_string(),
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let listener = UnixListener::bind(&self.socket_path)?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&self.socket_path, std::fs::Permissions::from_mode(0o666))?;
        info!("IPC server listening on {}", self.socket_path);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let state = self.state.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = Self::handle_connection(stream, state) {
                            error!("IPC connection error: {e}");
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept IPC connection: {e}");
                }
            }
        }
        Ok(())
    }

    fn handle_connection(stream: UnixStream, state: Arc<AppState>) -> anyhow::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut writer = stream;
        let mut line = String::new();

        while reader.read_line(&mut line)? > 0 {
            let msg: IpcMessage = match serde_json::from_str(line.trim()) {
                Ok(m) => m,
                Err(e) => {
                    warn!("Invalid IPC message: {e}");
                    line.clear();
                    continue;
                }
            };

            let response = match &msg.payload {
                IpcPayload::Request { request } => {
                    Self::handle_request(request, &state)
                }
                _ => Response::error("Unexpected message type"),
            };

            let reply = IpcMessage {
                id: msg.id,
                payload: IpcPayload::Response { response },
            };
            let mut reply_json = serde_json::to_string(&reply)?;
            reply_json.push('\n');
            writer.write_all(reply_json.as_bytes())?;
            writer.flush()?;
            line.clear();
        }

        Ok(())
    }

    fn handle_request(request: &Request, state: &Arc<AppState>) -> Response {
        debug!("Handling request: {:?}", request);
        match request {
            Request::Ping => Response::ok(json!("pong")),

            Request::ListTunnels => {
                match state.tunnel_store.list() {
                    Ok(tunnels) => Response::ok(tunnels),
                    Err(e) => Response::error(e.to_string()),
                }
            }

            Request::GetTunnel { name } => {
                match state.tunnel_store.get(name) {
                    Ok(config) => Response::ok(config),
                    Err(e) => Response::error(e.to_string()),
                }
            }

            Request::AddTunnel { config } => {
                match state.tunnel_store.add(config) {
                    Ok(()) => Response::ok(json!("added")),
                    Err(e) => Response::error(e.to_string()),
                }
            }

            Request::UpdateTunnel { config } => {
                let active = state.active_tunnel.blocking_read().clone();
                if active.as_ref() == Some(&config.name) {
                    return Response::error("Cannot update a connected tunnel. Disconnect first.");
                }
                match state.tunnel_store.update(config) {
                    Ok(()) => Response::ok(json!("updated")),
                    Err(e) => Response::error(e.to_string()),
                }
            }

            Request::DeleteTunnel { name } => {
                let active = state.active_tunnel.blocking_read().clone();
                if active.as_ref() == Some(name) {
                    return Response::error("Cannot delete a connected tunnel. Disconnect first.");
                }
                match state.tunnel_store.delete(name) {
                    Ok(()) => Response::ok(json!("deleted")),
                    Err(e) => Response::error(e.to_string()),
                }
            }

            Request::ConnectTunnel { name } => {
                let config = match state.tunnel_store.get(name) {
                    Ok(c) => c,
                    Err(e) => return Response::error(e.to_string()),
                };

                {
                    let mut active = state.active_tunnel.blocking_write();
                    if let Some(ref active_name) = *active {
                        let iface = state.wg_manager.interface_name(active_name);
                        if state.wg_manager.interface_exists(&iface) {
                            return Response::error(format!("Already connected to '{}'. Disconnect first.", active_name));
                        }
                        *active = None;
                    }
                }

                match state.wg_manager.connect(&config, &state.config.tunnel_dir) {
                    Ok(iface) => {
                        if let Err(e) = state.split_tunnel.on_tunnel_connected(&iface) {
                            warn!("Split tunnel on-connect failed: {e}");
                        }
                        let settings = state.config.settings.split_tunneling.clone();
                        if settings.enabled {
                            if let Err(e) = state.split_tunnel.apply_settings(&settings) {
                                warn!("Failed to re-apply split tunneling on connect: {e}");
                            }
                        }
                        let mut active = state.active_tunnel.blocking_write();
                        *active = Some(name.clone());
                        Response::ok(json!({
                            "interface": iface,
                            "name": name,
                        }))
                    }
                    Err(e) => Response::error(format!("Connection failed: {e}")),
                }
            }

            Request::DisconnectTunnel => {
                let active_name = {
                    let active = state.active_tunnel.blocking_read().clone();
                    match active {
                        Some(n) => n,
                        None => return Response::error("No active tunnel"),
                    }
                };

                let iface = format!("wg-{active_name}");

                if let Err(e) = state.split_tunnel.on_tunnel_disconnected() {
                    warn!("Split tunnel on-disconnect failed: {e}");
                }

                match state.wg_manager.disconnect(&iface) {
                    Ok(()) => {
                        let mut active = state.active_tunnel.blocking_write();
                        *active = None;
                        Response::ok(json!("disconnected"))
                    }
                    Err(e) => Response::error(format!("Disconnect failed: {e}")),
                }
            }

            Request::GetStatus => {
                let active_name = state.active_tunnel.blocking_read().clone();
                match active_name {
                    Some(name) => {
                        let iface = format!("wg-{name}");
                        let stats = state.wg_manager.get_stats(&iface).ok().flatten();
                        Response::ok(TunnelStatus {
                            name,
                            connected: true,
                            interface: iface,
                            stats,
                        })
                    }
                    None => Response::ok(TunnelStatus {
                        name: String::new(),
                        connected: false,
                        interface: String::new(),
                        stats: None,
                    }),
                }
            }

            Request::GetSettings => {
                Response::ok(&state.config.settings)
            }

            Request::UpdateSettings { settings } => {
                match state.config.save_settings(settings) {
                    Ok(()) => Response::ok(json!("saved")),
                    Err(e) => Response::error(e.to_string()),
                }
            }

            Request::SetSplitTunneling { settings } => {
                match state.split_tunnel.apply_settings(settings) {
                    Ok(()) => {
                        let mut s = state.config.settings.clone();
                        s.split_tunneling = settings.clone();
                        let _ = state.config.save_settings(&s);
                        Response::ok(json!("applied"))
                    }
                    Err(e) => Response::error(e.to_string()),
                }
            }

        Request::GenerateKeypair => {
            let (private, public) = crate::wireguard::WireGuardManager::generate_keypair();
            Response::ok(json!({
                "private_key": private,
                "public_key": public,
            }))
        }

        Request::ListProcesses => {
            let procs = crate::process_monitor::list_running_processes();
            Response::ok(procs)
        }
        }
    }
}
