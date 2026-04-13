const SOCKET_PATH = "/run/wgsplitd.sock";

let msgId = 0;

async function sendRequest(method: string, params?: Record<string, any>): Promise<any> {
  const request: Record<string, any> = { method };
  if (params) request.params = params;
  const msg = JSON.stringify({
    id: ++msgId,
    type: "request",
    request,
  });

  const proc = Bun.spawn(["socat", "-", `UNIX-CONNECT:${SOCKET_PATH}`], {
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
  });

  proc.stdin.write(msg + "\n");
  proc.stdin.flush();
  proc.stdin.end();

  const output = await new Response(proc.stdout).text();
  await proc.exited;

  if (!output.trim()) {
    throw new Error("Empty response from daemon");
  }

  const reply = JSON.parse(output.trim());

  if (reply.response?.status === "error") {
    throw new Error(reply.response.data);
  }
  return reply.response?.data ?? null;
}

export async function listTunnels() {
  return sendRequest("list_tunnels");
}
export async function getTunnel(name: string) {
  return sendRequest("get_tunnel", { name });
}
export async function addTunnel(config: any) {
  return sendRequest("add_tunnel", { config });
}
export async function updateTunnel(config: any) {
  return sendRequest("update_tunnel", { config });
}
export async function deleteTunnel(name: string) {
  return sendRequest("delete_tunnel", { name });
}
export async function connectTunnel(name: string) {
  return sendRequest("connect_tunnel", { name });
}
export async function disconnectTunnel() {
  return sendRequest("disconnect_tunnel");
}
export async function getStatus() {
  return sendRequest("get_status");
}
export async function getSettings() {
  return sendRequest("get_settings");
}
export async function updateSettings(settings: any) {
  return sendRequest("update_settings", { settings });
}
export async function setSplitTunneling(settings: any) {
  return sendRequest("set_split_tunneling", { settings });
}
export async function generateKeypair() {
  return sendRequest("generate_keypair");
}
export async function listProcesses() {
  return sendRequest("list_processes");
}
export async function checkDaemon() {
  return sendRequest("ping");
}
