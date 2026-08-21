import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CreateFrpTunnel,
  FrpClientStatus,
  FrpDownloadProgress,
  FrpNode,
  FrpProvider,
  FrpSessionStatus,
  FrpTunnel,
} from "@models/frp";

export interface FrpSnapshot {
  client: FrpClientStatus;
  session: FrpSessionStatus;
  tunnels: FrpTunnel[];
}

const snapshots = new Map<FrpProvider, FrpSnapshot>();
const preloadTasks = new Map<FrpProvider, Promise<void>>();
let restoreTask: Promise<FrpSessionStatus[]> | null = null;

export function getCachedFrpSnapshot(provider: FrpProvider): FrpSnapshot | null {
  return snapshots.get(provider) ?? null;
}

export function cacheFrpSnapshot(provider: FrpProvider, snapshot: FrpSnapshot): void {
  snapshots.set(provider, snapshot);
}

export function clearCachedFrpSnapshot(provider: FrpProvider): void {
  snapshots.delete(provider);
}

export async function preloadFrpProvider(provider: FrpProvider): Promise<void> {
  if (snapshots.has(provider)) return;
  const existing = preloadTasks.get(provider);
  if (existing) return existing;
  const task = (async () => {
    const client = await getFrpClientStatus(provider);
    const session = await restoreFrpSessions().then((sessions) =>
      sessions.find((candidate) => candidate.provider === provider)!,
    );
    const tunnels = session.authenticated ? await listFrpTunnels(provider) : [];
    cacheFrpSnapshot(provider, { client, session, tunnels });
  })();
  preloadTasks.set(provider, task);
  try {
    await task;
  } finally {
    preloadTasks.delete(provider);
  }
}

export function getFrpClientStatus(provider: FrpProvider): Promise<FrpClientStatus> {
  return invoke("get_frp_client_status", { provider });
}

export function downloadFrpClient(provider: FrpProvider): Promise<FrpClientStatus> {
  return invoke("download_frp_client", { provider });
}

export function onFrpDownloadProgress(
  handler: (progress: FrpDownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<FrpDownloadProgress>("frp-download-progress", ({ payload }) => handler(payload));
}

export function getFrpSessionStatus(provider: FrpProvider): Promise<FrpSessionStatus> {
  return invoke("get_frp_session_status", { provider });
}

export function restoreFrpSessions(): Promise<FrpSessionStatus[]> {
  if (!restoreTask) {
    restoreTask = invoke("restore_frp_sessions");
    void restoreTask.catch(() => {
      restoreTask = null;
    });
  }
  return restoreTask;
}

export function loginSakuraFrp(credential: string): Promise<FrpSessionStatus> {
  return invoke("login_sakurafrp", { credential });
}

export function loginOpenFrp(): Promise<FrpSessionStatus> {
  return invoke("login_openfrp");
}

export function openSakuraKeys(): Promise<void> {
  return invoke("open_sakura_keys");
}

export function openSakuraPurchase(): Promise<void> {
  return invoke("open_sakura_purchase");
}

export function openPremium(): Promise<void> {
  return invoke("open_premium");
}

export function logoutFrp(provider: FrpProvider): Promise<FrpSessionStatus> {
  return invoke("logout_frp", { provider });
}

export function listFrpTunnels(provider: FrpProvider): Promise<FrpTunnel[]> {
  return invoke("list_frp_tunnels", { provider });
}

export function listFrpNodes(provider: FrpProvider): Promise<FrpNode[]> {
  return invoke("list_frp_nodes", { provider });
}

export function createFrpTunnel(
  provider: FrpProvider,
  request: CreateFrpTunnel,
): Promise<FrpTunnel[]> {
  return invoke("create_frp_tunnel", { provider, request });
}

export function editFrpTunnel(
  provider: FrpProvider,
  request: CreateFrpTunnel & { tunnelId: string },
): Promise<FrpTunnel[]> {
  return invoke("edit_frp_tunnel", { provider, request });
}

export function deleteFrpTunnel(provider: FrpProvider, tunnelId: string): Promise<FrpTunnel[]> {
  return invoke("delete_frp_tunnel", { provider, tunnelId });
}

export function startFrpTunnel(provider: FrpProvider, tunnelId: string): Promise<FrpSessionStatus> {
  return invoke("start_frp_tunnel", { provider, tunnelId });
}

export function stopFrpTunnel(provider: FrpProvider): Promise<FrpSessionStatus> {
  return invoke("stop_frp_tunnel", { provider });
}
