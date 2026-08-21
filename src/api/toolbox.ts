import { invoke } from "@tauri-apps/api/core";

export interface NetworkDiagnostics {
  publicIpv4: string | null;
  publicIpv6: string | null;
  udpAvailable: boolean;
  mappingVariesByDestination: boolean | null;
  relayAvailable: boolean;
}

export interface RelayDiagnostics {
  relayUrl: string | null;
  latencyMs: number | null;
}

export function runNetworkDiagnostics(): Promise<NetworkDiagnostics> {
  return invoke("run_network_diagnostics");
}

export function runRelayDiagnostics(relayUrl: string | null): Promise<RelayDiagnostics> {
  return invoke("run_relay_diagnostics", { relayUrl });
}
