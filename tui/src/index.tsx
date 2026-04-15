import { createCliRenderer } from "@opentui/core";
import {
    createRoot,
    useKeyboard,
    useRenderer,
    useTerminalDimensions,
} from "@opentui/react";
import { useState, useEffect, useCallback, useRef } from "react";
import {
    listTunnels,
    connectTunnel,
    disconnectTunnel,
    deleteTunnel,
    getStatus,
    getSettings,
    setSplitTunneling,
    generateKeypair,
    getTunnel,
    addTunnel,
    updateTunnel,
    checkDaemon,
    listProcesses,
} from "./ipc";

type Screen =
    | "main"
    | "editor"
    | "split"
    | "add_app"
    | "add_domain"
    | "pick_app";

function formatBytes(b: number): string {
    if (b === 0) return "0 B";
    const k = 1024;
    const u = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(b) / Math.log(k));
    return (b / Math.pow(k, i)).toFixed(1) + " " + u[i];
}

function App() {
    const renderer = useRenderer();
    const { height: termHeight } = useTerminalDimensions();
    const [screen, setScreen] = useState<Screen>("main");
    const [status, setStatus] = useState<any>(null);
    const [tunnels, setTunnels] = useState<any[]>([]);
    const [error, setError] = useState("");
    const [selectedIdx, setSelectedIdx] = useState(0);
    const [settings, setSettings] = useState<any>(null);
    const [editingTunnel, setEditingTunnel] = useState<any>(null);
    const [daemonOk, setDaemonOk] = useState(false);
    const [showHelp, setShowHelp] = useState(false);
    const [pickAppRef, setPickAppRef] = useState<{
        procs: any[];
        filtered: any[];
        selectedIdx: number;
        filter: string;
        filtering: boolean;
        onSelect: (exe: string) => void;
        scrollOffset: number;
    }>({
        procs: [],
        filtered: [],
        selectedIdx: 0,
        filter: "",
        filtering: false,
        onSelect: () => {},
        scrollOffset: 0,
    });
    const [pendingApps, setPendingApps] = useState<{ vpn: string[]; direct: string[] } | null>(null);

    async function refresh() {
        try {
            const [s, tunnelNames, st] = await Promise.all([
                getStatus(),
                listTunnels(),
                getSettings(),
            ]);
            setStatus(s);
            const tunnelConfigs = await Promise.all(
                (tunnelNames || []).map((name: string) =>
                    getTunnel(name).catch(() => null),
                ),
            );
            setTunnels(tunnelConfigs.filter(Boolean));
            setSettings(st);
            setDaemonOk(true);
            setError("");
        } catch (e: any) {
            setDaemonOk(false);
            setError(e.message);
        }
    }

    useEffect(() => {
        refresh();
    }, []);
    useEffect(() => {
        const iv = setInterval(refresh, 3000);
        return () => clearInterval(iv);
    }, []);

    const connected = status?.connected || false;
    const connectedName = status?.name || "";
    const stats = status?.stats || null;

    useKeyboard((key) => {
        if (key.name === "?") {
            setShowHelp((h) => !h);
            return;
        }
        if (showHelp) {
            if (key.name !== "up" && key.name !== "down") {
                setShowHelp(false);
            }
            return;
        }

        if (screen === "pick_app") {
            if (pickAppRef.filtering) {
                if (key.name === "escape") {
                    setPickAppRef((r) => ({
                        ...r,
                        filtering: false,
                        filter: "",
                    }));
                } else if (
                    key.name === "return" &&
                    pickAppRef.filtered.length > 0
                ) {
                    pickAppRef.onSelect(
                        pickAppRef.filtered[pickAppRef.selectedIdx].exe,
                    );
                } else if (key.name === "backspace") {
                    const newFilter = pickAppRef.filter.slice(0, -1);
                    const filtered = newFilter
                        ? pickAppRef.procs.filter(
                              (p: any) =>
                                  p.name
                                      .toLowerCase()
                                      .includes(newFilter.toLowerCase()) ||
                                  p.exe
                                      .toLowerCase()
                                      .includes(newFilter.toLowerCase()),
                          )
                        : pickAppRef.procs;
                    setPickAppRef((r) => ({
                        ...r,
                        filter: newFilter,
                        filtered,
                        selectedIdx: Math.min(
                            r.selectedIdx,
                            filtered.length - 1,
                        ),
                    }));
                } else if (
                    key.name &&
                    key.name.length === 1 &&
                    /[a-z0-9]/.test(key.name)
                ) {
                    const newFilter =
                        pickAppRef.filter + key.name.toLowerCase();
                    const filtered = pickAppRef.procs.filter(
                        (p: any) =>
                            p.name.toLowerCase().includes(newFilter) ||
                            p.exe.toLowerCase().includes(newFilter),
                    );
                    setPickAppRef((r) => ({
                        ...r,
                        filter: newFilter,
                        filtered,
                        selectedIdx: Math.min(
                            r.selectedIdx,
                            filtered.length - 1,
                        ),
                    }));
                }
                return;
            }
            if (key.name === "escape") setScreen("split");
            if (key.name === "up") {
                setPickAppRef((r) => {
                    const newIdx = Math.max(0, r.selectedIdx - 1);
                    return { ...r, selectedIdx: newIdx };
                });
            }
            if (key.name === "down") {
                setPickAppRef((r) => {
                    const newIdx = Math.min(
                        r.filtered.length - 1,
                        r.selectedIdx + 1,
                    );
                    return { ...r, selectedIdx: newIdx };
                });
            }
            if (key.name === "return" && pickAppRef.filtered.length > 0) {
                pickAppRef.onSelect(
                    pickAppRef.filtered[pickAppRef.selectedIdx].exe,
                );
            }
            if (key.name === "/")
                setPickAppRef((r) => ({ ...r, filtering: true }));
            return;
        }

        if (key.name === "escape" || key.name === "q") {
            if (screen !== "main") {
                setScreen("main");
                return;
            }
            renderer.destroy();
            return;
        }
        if (key.ctrl && key.name === "c") {
            renderer.destroy();
            return;
        }

        if (screen === "main") {
            if (key.name === "up") setSelectedIdx((i) => Math.max(0, i - 1));
            if (key.name === "down")
                setSelectedIdx((i) => Math.min(tunnels.length - 1, i + 1));
            if (key.name === "return") {
                setScreen("split");
            }
            if (key.name === "c") {
                const tunnel = tunnels[selectedIdx];
                if (tunnel) {
                    if (connected && connectedName === tunnel.name) {
                        disconnectTunnel()
                            .then(refresh)
                            .catch((e) => setError(e.message));
                    } else if (!connected) {
                        connectTunnel(tunnel.name)
                            .then(refresh)
                            .catch((e) => setError(e.message));
                    }
                }
            }
            if (key.name === "e") {
                const tunnel = tunnels[selectedIdx];
                if (tunnel) {
                    setEditingTunnel(tunnel);
                    setScreen("editor");
                }
            }
            if (key.name === "d" && !connected) {
                const tunnel = tunnels[selectedIdx];
                if (tunnel) {
                    deleteTunnel(tunnel.name)
                        .then(refresh)
                        .catch((e) => setError(e.message));
                }
            }
            if (key.name === "a") {
                setEditingTunnel(null);
                setScreen("editor");
            }
            if (key.name === "s") setScreen("split");
            return;
        }
    });

    return (
        <box flexDirection="column" width="100%" height="100%">
            <box border borderColor="#7aa2f7" paddingX={1} flexShrink={0}>
                <box
                    flexDirection="row"
                    width="100%"
                    justifyContent="space-between"
                >
                    <text fg="#c0caf5">
                        <strong>WireGuard Split Tunneling Manager</strong>
                    </text>
                    <text fg={daemonOk ? "#9ece6a" : "#f7768e"}>
                        {daemonOk ? "READY" : "FAIL"}
                    </text>
                </box>
            </box>

            {error && (
                <box backgroundColor="#f7768e" paddingX={1}>
                    <text fg="#1a1b26">{error}</text>
                </box>
            )}

            {screen === "main" && (
                <TunnelList
                    tunnels={tunnels}
                    selectedIdx={selectedIdx}
                    connectedName={connectedName}
                    connected={connected}
                    stats={stats}
                    splitEnabled={settings?.split_tunneling?.enabled || false}
                    splitRoutes={status?.split_routes || 0}
                />
            )}
            {screen === "editor" && (
                <TunnelEditor
                    tunnel={editingTunnel}
                    onSave={async (config) => {
                        try {
                            if (editingTunnel) await updateTunnel(config);
                            else await addTunnel(config);
                            await refresh();
                            setSelectedIdx(0);
                            setScreen("main");
                        } catch (e: any) {
                            setError(e.message);
                        }
                    }}
                />
            )}
            {screen === "split" && settings && (
                <SplitTunnelView
                    settings={settings.split_tunneling}
                    connected={connected}
                    onSave={async (s) => {
                        try {
                            await setSplitTunneling(s);
                            await refresh();
                        } catch (e: any) {
                            setError(e.message);
                        }
                    }}
  onPickApp={async (currentVpnApps: string[], currentDirectApps: string[], currentMode: string) => {
    const p = await listProcesses();
    const procs = (p || []).sort((a: any, b: any) =>
      a.name.localeCompare(b.name),
    );
    setPickAppRef({
      procs,
      filtered: procs,
      selectedIdx: 0,
      filter: "",
      filtering: false,
      scrollOffset: 0,
      onSelect: (exe: string) => {
        if (currentMode === "inclusive") {
          setPendingApps({ vpn: [...currentVpnApps, exe], direct: currentDirectApps });
        } else {
          setPendingApps({ vpn: currentVpnApps, direct: [...currentDirectApps, exe] });
        }
        setScreen("split");
      },
    });
    setScreen("pick_app");
  }}
  pendingApps={pendingApps}
  onClearPendingApps={() => setPendingApps(null)}
                />
            )}
            {screen === "pick_app" && (
                <ProcessPicker
                    state={pickAppRef}
                    setState={setPickAppRef}
                    viewportHeight={Math.max(5, (termHeight || 24) - 8)}
                />
            )}

            <box border borderColor="#7aa2f7" paddingX={1} flexShrink={0}>
                <box
                    flexDirection="row"
                    width="100%"
                    justifyContent="space-between"
                >
                    {screen === "main" && (
                        <text fg="#c0caf5">
                            [c] connect [s] split [e] edit [a] add
                        </text>
                    )}
                    {screen === "editor" && (
                        <text fg="#c0caf5">
                            [↑/↓/tab] next [enter] save [esc] cancel
                        </text>
                    )}
                    {screen === "split" && <text fg="#c0caf5">[esc] back</text>}
                    {screen === "pick_app" && (
                        <text fg="#c0caf5">
                            [/] filter [enter] select [esc] cancel
                        </text>
                    )}
                    <text fg="#c0caf5">
                        [?] help {screen === "main" ? "[q] quit" : ""}
                    </text>
                </box>
            </box>

            {showHelp && (
                <box
                    position="absolute"
                    top={3}
                    left={5}
                    width={60}
                    height={Math.max(5, (termHeight || 24) - 7)}
                    border
                    borderColor="#7aa2f7"
                    backgroundColor="#1a1b26"
                >
                    <scrollbox focused height="100%">
                        <box flexDirection="column" padding={1}>
                            <text fg="#7aa2f7">
                                <strong>Keyboard Shortcuts</strong>
                            </text>
                            <text> </text>
                            <text fg="#9ece6a">
                                <strong>Tunnel List</strong>
                            </text>
                            <text fg="#c0caf5">
                                {" "}
                                [enter] open split tunneling
                            </text>
                            <text fg="#c0caf5"> [c] connect / disconnect</text>
                            <text fg="#c0caf5"> [e] edit tunnel</text>
                            <text fg="#c0caf5"> [a] add new tunnel</text>
                            <text fg="#c0caf5"> [d] delete tunnel</text>
                            <text fg="#c0caf5"> [↑/↓] navigate</text>
                            <text> </text>
                            <text fg="#9ece6a">
                                <strong>Split Tunneling</strong>
                            </text>
                            <text fg="#c0caf5"> [1] toggle enabled</text>
                            <text fg="#c0caf5"> [2] toggle mode</text>
                            <text fg="#c0caf5"> [a] add application</text>
                            <text fg="#c0caf5"> [d] add domain/IP</text>
                            <text fg="#c0caf5"> [x] remove selected</text>
                            <text fg="#c0caf5"> [s] save / apply</text>
                            <text> </text>
                            <text fg="#9ece6a">
                                <strong>Process Picker</strong>
                            </text>
                            <text fg="#c0caf5"> [/] filter</text>
                            <text fg="#c0caf5"> [↑/↓] navigate</text>
                            <text fg="#c0caf5"> [enter] select</text>
                            <text> </text>
                            <text fg="#9ece6a">
                                <strong>CLI</strong>
                            </text>
                            <text fg="#c0caf5"> wgsplit                        launch TUI</text>
                            <text fg="#c0caf5"> wgsplit import tunnel.conf     import tunnel</text>
                            <text> </text>
                            <text fg="#565f89"> [?] or any key to close</text>
                        </box>
                    </scrollbox>
                </box>
            )}
        </box>
    );
}

function formatHandshake(ts: number | null | undefined): string {
    if (!ts || ts === 0) return "never";
    const ago = Math.floor(Date.now() / 1000) - ts;
    if (ago < 0) return "just now";
    if (ago < 60) return `${ago}s ago`;
    if (ago < 3600) return `${Math.floor(ago / 60)}m ago`;
    return `${Math.floor(ago / 3600)}h ago`;
}

function TunnelList({
    tunnels,
    selectedIdx,
    connectedName,
    connected,
    stats,
    splitEnabled,
    splitRoutes,
}: {
    tunnels: any[];
    selectedIdx: number;
    connectedName: string;
    connected: boolean;
    stats: any;
    splitEnabled: boolean;
    splitRoutes: number;
}) {
    return (
        <scrollbox focused flexGrow={1} minHeight={3}>
            <box flexDirection="column" paddingX={1}>
                {tunnels.length === 0 && (
                    <text fg="#565f89">
                        No tunnels configured. Press [a] to add one.
                    </text>
                )}
                {tunnels.map((tunnel: any, i: number) => {
                    const isActive = connectedName === tunnel.name;
                    const isSelected = i === selectedIdx;
                    const peer = tunnel.peers?.[0];
                    const endpoint = peer?.endpoint || "";
                    const host = endpoint.split(":")[0] || "";
                    const addrs = tunnel.interface?.address?.join(", ") || "";
                    return (
                        <box
                            key={tunnel.name}
                            flexDirection="column"
                            backgroundColor={isSelected ? "#2f3349" : undefined}
                            paddingX={1}
                            marginBottom={1}
                        >
                            <box flexDirection="row" width="100%" justifyContent="space-between">
                                <box flexDirection="row">
                                    <text fg={isSelected ? "#7aa2f7" : "#c0caf5"}>
                                        {isSelected ? ">" : " "}
                                    </text>
                                    <text fg={isActive ? "#9ece6a" : "#c0caf5"}>
                                        <strong>{tunnel.name}</strong>
                                    </text>
                                </box>
                                {isActive && (
                                    <text fg="#9ece6a">● CONNECTED</text>
                                )}
                            </box>
                            <box flexDirection="row" width="100%" justifyContent="space-between" paddingLeft={2}>
                                <text fg="#a9b1d6">
                                    {host ? `endpoint: ${host}` : ""}
                                    {addrs ? `  addr: ${addrs}` : ""}
                                    {peer?.allowed_ips ? `  routes: ${peer.allowed_ips.join(", ")}` : ""}
                                </text>
                            </box>
                            {isActive && stats && (
                                <box flexDirection="row" width="100%" justifyContent="space-between" paddingLeft={2}>
                                    <text fg="#a9b1d6">
                                        ↓{formatBytes(stats.rx_bytes)}  ↑{formatBytes(stats.tx_bytes)}  handshake: {formatHandshake(stats.latest_handshake)}
                                    </text>
                                    {splitEnabled && splitRoutes > 0 && (
                                        <text fg="#7aa2f7">{splitRoutes} split routes</text>
                                    )}
                                </box>
                            )}
                        </box>
                    );
                })}
            </box>
        </scrollbox>
    );
}

function TunnelEditor({
    tunnel,
    onSave,
}: {
    tunnel: any;
    onSave: (config: any) => void;
}) {
    const fields = [
        "name",
        "privateKey",
        "address",
        "dns",
        "peerPublicKey",
        "peerEndpoint",
        "peerAllowedIps",
        "peerKeepalive",
    ];
    const isEdit = !!tunnel;

    const initialValues: Record<string, string> = {
        name: tunnel?.name || "",
        privateKey: tunnel?.interface?.private_key || "",
        address: tunnel?.interface?.address?.join(", ") || "",
        dns: tunnel?.interface?.dns?.join(", ") || "",
        peerPublicKey: tunnel?.peers?.[0]?.public_key || "",
        peerEndpoint: tunnel?.peers?.[0]?.endpoint || "",
        peerAllowedIps:
            tunnel?.peers?.[0]?.allowed_ips?.join(", ") || "0.0.0.0/0",
        peerKeepalive:
            tunnel?.peers?.[0]?.persistent_keepalive?.toString() || "25",
    };
    const [values, setValues] = useState<Record<string, string>>(initialValues);
    const [focusIdx, setFocusIdx] = useState(0);

    useKeyboard((key) => {
        if (key.name === "tab" || key.name === "down") {
            setFocusIdx((i) => (i + 1) % fields.length);
        }
        if (key.name === "up") {
            setFocusIdx((i) => (i - 1 + fields.length) % fields.length);
        }
        if (key.name === "return") {
            const config = {
                name: values.name,
                interface: {
                    private_key: values.privateKey,
                    address: values.address
                        .split(",")
                        .map((s: string) => s.trim())
                        .filter(Boolean),
                    dns: values.dns
                        ? values.dns
                              .split(",")
                              .map((s: string) => s.trim())
                              .filter(Boolean)
                        : [],
                    table: "off",
                },
                peers: [
                    {
                        public_key: values.peerPublicKey,
                        endpoint: values.peerEndpoint,
                        allowed_ips: values.peerAllowedIps
                            .split(",")
                            .map((s: string) => s.trim())
                            .filter(Boolean),
                        persistent_keepalive: values.peerKeepalive
                            ? parseInt(values.peerKeepalive)
                            : null,
                    },
                ],
            };
            onSave(config);
        }
    });

    const labels: Record<string, string> = {
        name: "Tunnel Name",
        privateKey: "Private Key",
        address: "Address",
        dns: "DNS",
        peerPublicKey: "Peer Public Key",
        peerEndpoint: "Peer Endpoint",
        peerAllowedIps: "Allowed IPs",
        peerKeepalive: "Keepalive",
    };

    return (
        <box flexDirection="column" flexGrow={1} paddingX={1}>
            <text fg="#7aa2f7">
                <strong>{isEdit ? "Edit Tunnel" : "Add Tunnel"}</strong>
            </text>
            <box flexDirection="column">
                {fields.map((field, i) => (
                    <box key={field} flexDirection="row" gap={1}>
                        <text fg={i === focusIdx ? "#7aa2f7" : "#565f89"}>
                            {i === focusIdx ? ">" : " "}
                            {labels[field]}:
                        </text>
                        {isEdit && field === "name" ? (
                            <text fg="#c0caf5">{values[field]}</text>
                        ) : (
                            <input
                                value={values[field]}
                                onChange={(v: string) =>
                                    setValues((prev) => ({
                                        ...prev,
                                        [field]: v,
                                    }))
                                }
                                focused={i === focusIdx}
                                width={50}
                                backgroundColor="#1a1b26"
                            />
                        )}
                    </box>
                ))}
                <box flexDirection="row" gap={2}>
                    <box
                        border
                        onMouseDown={() => {
                            generateKeypair().then((keys) =>
                                setValues((v) => ({
                                    ...v,
                                    privateKey: keys.private_key,
                                })),
                            );
                        }}
                    >
                        <text>Generate Keys</text>
                    </box>
                </box>
            </box>
        </box>
    );
}

function SplitTunnelView({
    settings,
    connected,
    onSave,
    onPickApp,
    pendingApps,
    onClearPendingApps,
}: {
    settings: any;
    connected: boolean;
    onSave: (s: any) => Promise<void>;
    onPickApp: (
        currentVpnApps: string[],
        currentDirectApps: string[],
        currentMode: string,
    ) => void;
    pendingApps?: { vpn: string[]; direct: string[] } | null;
    onClearPendingApps?: () => void;
}) {
    const settingsVpn = settings?.apps?.vpn || [];
    const settingsDirect = settings?.apps?.direct || [];
    const settingsVpnDomains = settings?.domains?.vpn || [];
    const settingsDirectDomains = settings?.domains?.direct || [];

    const [localOverrides, setLocalOverrides] = useState<{
        enabled?: boolean;
        mode?: string;
        vpnApps?: string[];
        directApps?: string[];
        vpnDomains?: string[];
        directDomains?: string[];
    }>(() =>
        pendingApps
            ? { vpnApps: pendingApps.vpn, directApps: pendingApps.direct }
            : {},
    );
    useEffect(() => {
        if (pendingApps) onClearPendingApps?.();
    }, []);

    const enabled = localOverrides.enabled ?? settings?.enabled ?? false;
    const mode = localOverrides.mode ?? settings?.mode ?? "exclusive";
    const vpnApps: string[] = localOverrides.vpnApps ?? settingsVpn;
    const directApps: string[] = localOverrides.directApps ?? settingsDirect;
    const vpnDomains: string[] = localOverrides.vpnDomains ?? settingsVpnDomains;
    const directDomains: string[] = localOverrides.directDomains ?? settingsDirectDomains;

    const [newApp, setNewApp] = useState("");
    const [newDomain, setNewDomain] = useState("");
    const [inputMode, setInputMode] = useState<"none" | "app" | "domain">(
        "none",
    );
    const [focusIdx, setFocusIdx] = useState(0);
    const [appSelIdx, setAppSelIdx] = useState(0);
    const [domainSelIdx, setDomainSelIdx] = useState(0);

    const allApps = [...directApps.map((a) => ({ t: "d" as const, v: a })), ...vpnApps.map((a) => ({ t: "v" as const, v: a }))];
    const allDomains = [...directDomains.map((d) => ({ t: "d" as const, v: d })), ...vpnDomains.map((d) => ({ t: "v" as const, v: d }))];

    useKeyboard((key) => {
        if (inputMode === "none") {
            if (key.name === "1") {
                setLocalOverrides((o) => ({ ...o, enabled: !enabled }));
            }
            if (key.name === "2") {
                setLocalOverrides((o) => ({
                    ...o,
                    mode: mode === "exclusive" ? "inclusive" : "exclusive",
                }));
            }
            if (key.name === "a") {
                onPickApp(vpnApps, directApps, mode);
            }
            if (key.name === "d") {
                setInputMode("domain");
                setFocusIdx(1);
            }
            if (key.name === "x") {
                if (focusIdx === 0 && allApps.length > 0) {
                    const idx = Math.min(appSelIdx, allApps.length - 1);
                    const sel = allApps[idx]!;
                    if (sel.t === "d") {
                        setLocalOverrides((o) => ({
                            ...o,
                            directApps: directApps.filter((a) => a !== sel.v),
                        }));
                    } else {
                        setLocalOverrides((o) => ({
                            ...o,
                            vpnApps: vpnApps.filter((a) => a !== sel.v),
                        }));
                    }
                    setAppSelIdx((i) => Math.max(0, Math.min(i, allApps.length - 2)));
                } else if (focusIdx === 1 && allDomains.length > 0) {
                    const idx = Math.min(domainSelIdx, allDomains.length - 1);
                    const sel = allDomains[idx]!;
                    if (sel.t === "d") {
                        setLocalOverrides((o) => ({
                            ...o,
                            directDomains: directDomains.filter((d) => d !== sel.v),
                        }));
                    } else {
                        setLocalOverrides((o) => ({
                            ...o,
                            vpnDomains: vpnDomains.filter((d) => d !== sel.v),
                        }));
                    }
                    setDomainSelIdx((i) => Math.max(0, Math.min(i, allDomains.length - 2)));
                }
            }
            if (key.name === "s") {
                onSave({
                    enabled,
                    mode,
                    apps: { vpn: vpnApps, direct: directApps },
                    domains: {
                        vpn: vpnDomains,
                        direct: directDomains,
                        resolve_interval_secs: 300,
                    },
                }).then(() => {
                    setLocalOverrides({});
                });
            }
        } else {
            if (key.name === "return") {
                if (inputMode === "app" && newApp.trim()) {
                    if (mode === "exclusive")
                        setLocalOverrides((o) => ({
                            ...o,
                            directApps: [...directApps, newApp.trim()],
                        }));
                    else
                        setLocalOverrides((o) => ({
                            ...o,
                            vpnApps: [...vpnApps, newApp.trim()],
                        }));
                    setNewApp("");
                    setInputMode("none");
                }
                if (inputMode === "domain" && newDomain.trim()) {
                    setLocalOverrides((o) => ({
                        ...o,
                        vpnDomains: [...vpnDomains, newDomain.trim()],
                    }));
                    setNewDomain("");
                    setInputMode("none");
                }
            }
            if (key.name === "escape") {
                setInputMode("none");
            }
        }
            if (key.name === "tab") setFocusIdx((i) => (i === 0 ? 1 : 0));
            if (inputMode === "none") {
                if (key.name === "up") {
                    if (focusIdx === 0) setAppSelIdx((i) => Math.max(0, i - 1));
                    else setDomainSelIdx((i) => Math.max(0, i - 1));
                }
                if (key.name === "down") {
                    if (focusIdx === 0) setAppSelIdx((i) => Math.min(allApps.length - 1, i + 1));
                    else setDomainSelIdx((i) => Math.min(allDomains.length - 1, i + 1));
                }
            }
        });

    if (!connected) {
        return (
            <box flexGrow={1} alignItems="center" justifyContent="center">
                <text fg="#565f89">
                    Connect to a tunnel first to configure split tunneling
                </text>
            </box>
        );
    }

    return (
        <scrollbox focused flexGrow={1} minHeight={3}>
            <box flexDirection="column" paddingX={1}>
                <text fg="#7aa2f7">
                    <strong>Split Tunneling</strong>
                </text>

                <box flexDirection="row" gap={2}>
                    <text fg={enabled ? "#9ece6a" : "#f7768e"}>
                        [1] {enabled ? "ENABLED" : "DISABLED"}
                    </text>
                    <text fg="#c0caf5">[2] Mode: {mode}</text>
                    <text fg="#9ece6a">[s] save/apply</text>
                </box>

                <text fg="#565f89">
                        {mode === "exclusive"
                            ? "Exclusive: listed apps/domains/IPs bypass VPN"
                            : "Inclusive: only listed apps/domains/IPs use VPN"}
                </text>

                <text> </text>

                <box border borderColor={focusIdx === 0 ? "#7aa2f7" : "#3b4261"} paddingX={1}>
                    <box flexDirection="column">
                        <text fg="#7aa2f7">
                            <strong>Applications</strong> [a] add [x] remove
                        </text>
                        {allApps.map((app, i) => (
                            <box key={app.t + i} backgroundColor={focusIdx === 0 && i === appSelIdx ? "#2f3349" : undefined}>
                                <text fg={app.t === "d" ? "#9ece6a" : "#7aa2f7"}>
                                    {focusIdx === 0 && i === appSelIdx ? "> " : "  "}{app.t === "d" ? "direct" : "vpn"}: {app.v.split("/").pop()}
                                </text>
                            </box>
                        ))}
                        {allApps.length === 0 && (
                            <text fg="#565f89">No apps configured</text>
                        )}
                    </box>
                </box>

                <box border borderColor={focusIdx === 1 ? "#7aa2f7" : "#3b4261"} paddingX={1}>
                    <box flexDirection="column">
                        <text fg="#7aa2f7">
                            <strong>Domains / IPs</strong> [d] add [x] remove
                        </text>
                        {allDomains.map((d, i) => (
                            <box key={d.t + i} backgroundColor={focusIdx === 1 && i === domainSelIdx ? "#2f3349" : undefined}>
                                <text fg={d.t === "d" ? "#9ece6a" : "#7aa2f7"}>
                                    {focusIdx === 1 && i === domainSelIdx ? "> " : "  "}{d.t === "d" ? "direct" : "vpn"}: {d.v}
                                </text>
                            </box>
                        ))}
                            {vpnDomains.length === 0 &&
                            directDomains.length === 0 && (
                                <text fg="#565f89">No domains or IPs configured</text>
                            )}
                    </box>
                </box>

                {inputMode === "app" && (
                    <box flexDirection="row" gap={1}>
                        <text fg="#7aa2f7">App path:</text>
                        <input
                            value={newApp}
                            onChange={setNewApp}
                            focused
                            width={40}
                            backgroundColor="#1a1b26"
                        />
                    </box>
                )}
                {inputMode === "domain" && (
                    <box flexDirection="row" gap={1}>
                        <text fg="#7aa2f7">Domain/IP:</text>
                        <input
                            value={newDomain}
                            onChange={setNewDomain}
                            focused
                            width={40}
                            backgroundColor="#1a1b26"
                        />
                    </box>
                )}
            </box>
        </scrollbox>
    );
}

function ProcessPicker({
    state,
    setState,
    viewportHeight,
}: {
    state: {
        procs: any[];
        filtered: any[];
        selectedIdx: number;
        filter: string;
        filtering: boolean;
        onSelect: (exe: string) => void;
        scrollOffset: number;
    };
    setState: (fn: (prev: any) => any) => void;
    viewportHeight: number;
}) {
    const maxStart = Math.max(0, state.filtered.length - viewportHeight);
    const start = Math.max(
        0,
        Math.min(state.selectedIdx - Math.floor(viewportHeight / 2), maxStart),
    );
    const visibleItems = state.filtered.slice(start, start + viewportHeight);

    return (
        <box flexDirection="column" paddingX={1}>
            <box height={1}>
                <text fg="#7aa2f7">
                    <strong>Running Processes</strong> [/] filter
                </text>
            </box>
            {state.filtering ? (
                <box height={1} flexDirection="row" gap={1}>
                    <text fg="#7aa2f7">filter:</text>
                    <text fg="#c0caf5">{state.filter}_</text>
                </box>
            ) : (
                <box height={1}>
                    <text fg="#565f89">{state.filtered.length} processes</text>
                </box>
            )}
            <box
                flexDirection="column"
                height={viewportHeight}
                overflow="hidden"
            >
                {visibleItems.map((p: any, i: number) => {
                    const idx = start + i;
                    const isSelected = idx === state.selectedIdx;
                    return (
                        <box
                            key={p.pid}
                            flexDirection="row"
                            gap={1}
                            height={1}
                            backgroundColor={isSelected ? "#2f3349" : undefined}
                        >
                            <text fg={isSelected ? "#7aa2f7" : "#c0caf5"}>
                                {isSelected ? ">" : " "}
                            </text>
                            <text fg="#c0caf5">{p.name}</text>
                            <text fg="#565f89" flexGrow={1}>
                                {p.exe}
                            </text>
                        </box>
                    );
                })}
                {Array.from(
                    {
                        length: Math.max(
                            0,
                            viewportHeight - visibleItems.length,
                        ),
                    },
                    (_, i) => (
                        <box key={"blank" + i} height={1}>
                            <text> </text>
                        </box>
                    ),
                )}
            </box>
        </box>
    );
}

const args = process.argv.slice(2);
if (args.includes("--help") || args.includes("-h")) {
    console.log(`wgsplit - WireGuard Split Tunneling Manager

Usage:
  wgsplit                  Launch TUI
  wgsplit import <file>    Import tunnel from WireGuard .conf file
  wgsplit --help           Show this help

Service:
  sudo systemctl start wgsplitd   Start daemon
  sudo systemctl enable wgsplitd  Start on boot

Files:
  ~/.config/wgsplit/settings.toml   Daemon settings
  ~/.config/wgsplit/tunnels/        Tunnel configs`);
    process.exit(0);
}
if (args[0] === "import" && args[1]) {
    const fs = await import("fs/promises");
    const path = await import("path");
    const filePath = args[1];
    try {
        const contents = await fs.readFile(filePath, "utf-8");
        let name = path.basename(filePath, path.extname(filePath))
            .toUpperCase()
            .replace(/[^A-Z0-9_-]/g, "") || "imported";
        const msg = JSON.stringify({
            id: 1,
            type: "request",
            request: {
                method: "import_tunnel",
                params: { name, config_text: contents },
            },
        });
        const proc = Bun.spawn(["socat", "-", "UNIX-CONNECT:/run/wgsplitd.sock"], {
            stdin: "pipe",
            stdout: "pipe",
            stderr: "pipe",
        });
        proc.stdin.write(msg + "\n");
        proc.stdin.end();
        const out = await new Response(proc.stdout).text();
        const resp = JSON.parse(out.trim());
        if (resp.response?.status === "ok") {
            console.log(`imported tunnel '${name}'`);
        } else {
            console.error(`error: ${resp.response?.data || "unknown error"}`);
            process.exit(1);
        }
    } catch (e: any) {
        console.error(`error: ${e.message}`);
        process.exit(1);
    }
    process.exit(0);
}

const renderer = await createCliRenderer({ exitOnCtrlC: false });
createRoot(renderer).render(<App />);
