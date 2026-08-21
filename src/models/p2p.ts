export type P2pPhase = "idle" | "starting" | "active" | "stopping";

export type P2pErrorCode =
  | "invalid_join_uri"
  | "invalid_endpoint"
  | "authorization_denied"
  | "host_unreachable"
  | "target_unavailable"
  | "local_port_unavailable"
  | "identity_unavailable"
  | "operation_conflict"
  | "resource_limit"
  | "invalid_configuration"
  | "internal"
  | "unknown";

export interface P2pPeer {
  id: string;
  route: "direct" | "relay" | null;
  rttMs: number | null;
}

export interface P2pStatus {
  phase: P2pPhase;
  mode: "host" | "join" | null;
  localAddress: string | null;
  shareUri: string | null;
  playerCount: number;
  hostPort: number | null;
  route: "direct" | "relay" | null;
  rttMs: number | null;
  txBytes: number;
  rxBytes: number;
  hostPeers: P2pPeer[];
  error: P2pErrorCode | null;
  message: string | null;
}

export const emptyP2pStatus: P2pStatus = {
  phase: "idle",
  mode: null,
  localAddress: null,
  shareUri: null,
  playerCount: 0,
  hostPort: null,
  route: null,
  rttMs: null,
  txBytes: 0,
  rxBytes: 0,
  hostPeers: [],
  error: null,
  message: null,
};
