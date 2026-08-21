import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { HostUriLifetime } from "@models/preferences";
import type { P2pStatus } from "@models/p2p";

export interface LanScanSnapshot {
  scanning: boolean;
  port: number | null;
}

export function getP2pStatus(): Promise<P2pStatus> {
  return invoke("get_p2p_status");
}

export function onP2pStatus(callback: (status: P2pStatus) => void): Promise<() => void> {
  return listen<P2pStatus>("p2p-status", (event) => callback(event.payload));
}

export function startLanScan(restart = false): Promise<LanScanSnapshot> {
  return invoke(restart ? "restart_lan_scan" : "start_lan_scan");
}

export function getLanScan(): Promise<LanScanSnapshot> {
  return invoke("get_lan_scan");
}

export function stopLanScan(): Promise<void> {
  return invoke("stop_lan_scan");
}

export function probeHostPort(port: number): Promise<boolean> {
  return invoke("probe_host_port", { port });
}

export function startHost(
  port: number,
  maxPlayers: number | null,
  uriLifetime: HostUriLifetime,
): Promise<void> {
  return invoke("start_host", { port, maxPlayers, uriLifetime });
}

export function validateInvite(uri: string): Promise<void> {
  return invoke("validate_invite", { uri });
}

export function startJoin(uri: string, localPort: number | null): Promise<void> {
  return invoke("start_join", { uri, localPort });
}

export function saveJoinPort(port: number): Promise<void> {
  return invoke("set_join_port", { port });
}

export function stopJoin(): Promise<void> {
  return invoke("stop_join");
}

export function stopTunnel(): Promise<void> {
  return invoke("stop_tunnel");
}
