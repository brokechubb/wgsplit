# wgsplit

WireGuard Split Tunneling Manager for Linux.

Per-application and per-domain split tunneling with a terminal UI.

## Features

- **Per-application routing**: Route specific apps through VPN or direct
- **Per-domain routing**: Route specific domains through VPN or direct  
- **WireGuard management**: Add, edit, delete, connect/disconnect tunnels
- **Terminal UI**: Fast, responsive TUI built with OpenTUI
- **Systemd integration**: Run as a system service

## Requirements

- Linux with cgroups v2 (Arch, Fedora, modern distros)
- WireGuard (`wireguard-tools` package)
- `socat` for IPC communication
- `nftables` for packet marking
- Root privileges for the daemon

## Installation

### From Source

```bash
# Build daemon
cargo build --release

# Build TUI (requires Bun)
cd tui && bun build src/index.tsx --compile --outfile wgsplit

# Install
sudo cp target/release/wgsplitd /usr/local/bin/
sudo cp tui/wgsplit /usr/local/bin/
sudo cp contrib/wgsplitd.service /etc/systemd/system/
sudo systemctl daemon-reload
```

### Dependencies

```bash
# Arch
sudo pacman -S wireguard-tools socat nftables

# Fedora  
sudo dnf install wireguard-tools socat nftables
```

## Usage

### Start the daemon

```bash
sudo systemctl start wgsplitd
sudo systemctl enable wgsplitd  # optional: start on boot
```

### Launch the TUI

```bash
wgsplit
```

### Import a tunnel

```bash
wgsplit import /path/to/tunnel.conf
```

### Command line help

```bash
wgsplit --help
```

## TUI Keybindings

| Key | Action |
|-----|--------|
| `↑/↓` | Navigate tunnel list |
| `c` | Connect/disconnect |
| `s` | Open split tunneling config |
| `e` | Edit tunnel |
| `a` | Add new tunnel |
| `d` | Delete tunnel |
| `?` | Show help |
| `q` | Quit |

## Split Tunneling

### Per-Application

Add application executables to route through VPN (inclusive mode) or bypass VPN (exclusive mode). Uses cgroups v2 and nftables packet marking.

### Per-Domain  

Add domains to resolve and route through VPN or direct. DNS resolution updates automatically when IPs change.

## Configuration

- `~/.config/wgsplit/settings.toml` - Daemon settings
- `~/.config/wgsplit/tunnels/` - Tunnel configurations (WireGuard .conf format)

## Architecture

```
wgsplitd (daemon, runs as root)
├── IPC server on /run/wgsplitd.sock
├── WireGuard interface management
├── cgroups v2 process tracking
├── nftables fwmark marking
└── DNS resolution for domain routing

wgsplit (TUI)
└── Connects to daemon via Unix socket
```

## License

GPL-2.0
