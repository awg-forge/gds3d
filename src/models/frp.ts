export type FrpProvider = "open_frp" | "sakura_frp";

export interface FrpClientStatus {
  provider: FrpProvider;
  installed: boolean;
  downloading: boolean;
  path: string;
  error: string | null;
}

export interface FrpDownloadProgress {
  provider: FrpProvider;
  downloadedBytes: number;
  totalBytes: number | null;
  percent: number;
}

export interface FrpSessionStatus {
  provider: FrpProvider;
  authenticated: boolean;
  accountName: string | null;
  running: boolean;
  connected: boolean;
  tunnelId: string | null;
  output: string[];
}

export interface FrpTunnel {
  id: string;
  name: string;
  node: string | null;
  localPort: number | null;
  remoteEndpoint: string | null;
  online: boolean;
}

export interface FrpNode {
  id: string;
  name: string;
  description: string | null;
  vip: boolean;
  allowPort: string | null;
}

export interface CreateFrpTunnel {
  tunnelId?: string;
  nodeId: string;
  name: string;
  localPort: number;
  remotePort: string;
}
