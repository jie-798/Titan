import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export type EngineStatus = "stopped" | "starting" | "running" | "stopping" | "error";
export type SystemProxyStatus = "enabled" | "disabled" | "error";

export interface ProxyNode {
  id: string;
  name: string;
  protocol: string;
  server: string;
  port: number;
  latency: number | null;
  status: "active" | "inactive" | "error";
}

export interface ProxyGroup {
  id: string;
  name: string;
  type: "selector" | "url-test" | "fallback";
  currentNodeId: string | null;
  nodes: ProxyNode[];
}

export interface Session {
  id: string;
  sourceIp: string;
  sourcePort: number;
  destinationHost: string;
  destinationPort: number;
  proxyNodeId: string | null;
  ruleId: string | null;
  uploadBytes: number;
  downloadBytes: number;
  startTime: number;
  status: "active" | "closed";
}

export interface TrafficStats {
  uploadBytesPerSecond: number;
  downloadBytesPerSecond: number;
  totalUploadBytes: number;
  totalDownloadBytes: number;
  activeConnections: number;
}

export interface AppStatus {
  engineRunning: boolean;
  systemProxyEnabled: boolean;
  inboundBind: string;
  inboundPort: number;
  activeNode: string | null;
  error: string | null;
}

export interface GeoipInfo {
  version: string;
  lastUpdated: string | null;
}

export interface GeositeInfo {
  version: string;
  lastUpdated: string | null;
}

export interface ConfigSettings {
  inboundBind: string;
  inboundPort: number;
  apiBind: string;
  apiPort: number;
  autoStartEngine: boolean;
  autoEnableSystemProxy: boolean;
  configPath: string;
}

export interface SubscriptionUsage {
  fetchedAtUnix: number;
  fetchedAtLocal: string | null;
  upload: number | null;
  download: number | null;
  used: number | null;
  total: number | null;
  remaining: number | null;
  usagePercent: number | null;
  expire: number | null;
  expireLocal: string | null;
  source: string | null;
}

export interface ProxyTestItem {
  requestedName: string;
  resolvedName: string | null;
  status: "ok" | "failed" | "skipped";
  latencyMs: number | null;
  message: string | null;
}

export interface ProxyTestReport {
  modeLabel: string;
  scopeLabel: string;
  testUrl: string;
  timeoutMs: number;
  elapsedMs: number;
  ok: number;
  failed: number;
  skipped: number;
  items: ProxyTestItem[];
}

export interface ProfileSummary {
  configPath: string;
  mode: string;
  logLevel: string;
  proxyCount: number;
  proxyGroupCount: number;
  ruleCount: number;
  udpProxyCount: number;
  manualSelections: string[];
  rulePreview: string[];
  subscription: SubscriptionUsage | null;
}

interface AppState {
  status: AppStatus;
  proxies: ProxyGroup[];
  sessions: Session[];
  closedSessions: Session[];
  traffic: TrafficStats;
  geoip: GeoipInfo;
  geosite: GeositeInfo;
  config: ConfigSettings;
  profileSummary: ProfileSummary;
  testReport: ProxyTestReport | null;
  logs: string[];
  isLoading: boolean;
  error: string | null;

  fetchStatus: () => Promise<void>;
  fetchProxies: () => Promise<void>;
  fetchSessions: (stateFilter?: "active" | "closed" | "all", limit?: number) => Promise<void>;
  fetchClosedSessions: () => Promise<void>;
  fetchTraffic: () => Promise<void>;
  fetchConfig: () => Promise<void>;
  fetchProfileSummary: () => Promise<void>;
  fetchLogs: () => Promise<void>;
  startEngine: () => Promise<void>;
  stopEngine: () => Promise<void>;
  toggleSystemProxy: () => Promise<void>;
  selectProxy: (groupId: string, nodeId: string) => Promise<void>;
  testProxy: (nodeId: string) => Promise<ProxyTestReport>;
  testAllProxies: (groupId: string) => Promise<ProxyTestReport>;
  closeAllSessions: () => Promise<void>;
  clearSessionHistory: () => Promise<void>;
  updateConfig: (config: Partial<ConfigSettings>) => Promise<void>;
  clearError: () => void;
}

const toErrorMessage = (value: unknown): string => {
  if (value instanceof Error) {
    return value.message;
  }
  return String(value);
};

export const useAppStore = create<AppState>((set, get) => ({
  status: {
    engineRunning: false,
    systemProxyEnabled: false,
    inboundBind: "127.0.0.1",
    inboundPort: 7890,
    activeNode: null,
    error: null,
  },
  proxies: [],
  sessions: [],
  closedSessions: [],
  traffic: {
    uploadBytesPerSecond: 0,
    downloadBytesPerSecond: 0,
    totalUploadBytes: 0,
    totalDownloadBytes: 0,
    activeConnections: 0,
  },
  geoip: { version: "Unknown", lastUpdated: null },
  geosite: { version: "Unknown", lastUpdated: null },
  config: {
    inboundBind: "127.0.0.1",
    inboundPort: 7890,
    apiBind: "127.0.0.1",
    apiPort: 9090,
    autoStartEngine: false,
    autoEnableSystemProxy: false,
    configPath: "data/config.yaml",
  },
  profileSummary: {
    configPath: "data/config.yaml",
    mode: "rule",
    logLevel: "info",
    proxyCount: 0,
    proxyGroupCount: 0,
    ruleCount: 0,
    udpProxyCount: 0,
    manualSelections: [],
    rulePreview: [],
    subscription: null,
  },
  testReport: null,
  logs: [],
  isLoading: false,
  error: null,

  fetchStatus: async () => {
    try {
      const status = await invoke<AppStatus>("get_status");
      set({ status, error: null });
    } catch (e) {
      set({ error: toErrorMessage(e) });
    }
  },

  fetchProxies: async () => {
    try {
      const proxies = await invoke<ProxyGroup[]>("get_proxies");
      set({ proxies, error: null });
    } catch (e) {
      set({ error: toErrorMessage(e) });
    }
  },

  fetchSessions: async (stateFilter: "active" | "closed" | "all" = "active", limit = 100) => {
    try {
      const sessions = await invoke<Session[]>("get_sessions", { stateFilter, limit });
      if (stateFilter === "closed") {
        set({ closedSessions: sessions, error: null });
      } else if (stateFilter === "all") {
        set({ sessions, error: null });
      } else {
        set({ sessions, error: null });
      }
    } catch (e) {
      set({ error: toErrorMessage(e) });
    }
  },

  fetchClosedSessions: async () => {
    try {
      const sessions = await invoke<Session[]>("get_sessions", { stateFilter: "closed", limit: 200 });
      set({ closedSessions: sessions, error: null });
    } catch (e) {
      set({ error: toErrorMessage(e) });
    }
  },

  fetchTraffic: async () => {
    try {
      const traffic = await invoke<TrafficStats>("get_traffic");
      set({ traffic, error: null });
    } catch (e) {
      set({ error: toErrorMessage(e) });
    }
  },

  fetchConfig: async () => {
    try {
      const config = await invoke<ConfigSettings>("get_config");
      set({ config, error: null });
    } catch (e) {
      set({ error: toErrorMessage(e) });
    }
  },

  fetchProfileSummary: async () => {
    try {
      const profileSummary = await invoke<ProfileSummary>("get_profile_summary");
      set({ profileSummary, error: null });
    } catch (e) {
      set({ error: toErrorMessage(e) });
    }
  },

  fetchLogs: async () => {
    try {
      const logs = await invoke<string[]>("get_recent_logs", { limit: 120 });
      set({ logs, error: null });
    } catch (e) {
      set({ error: toErrorMessage(e) });
    }
  },

  startEngine: async () => {
    set({ isLoading: true, error: null });
    try {
      await invoke("start_engine");
      await Promise.all([
        get().fetchStatus(),
        get().fetchProxies(),
        get().fetchTraffic(),
        get().fetchSessions(),
        get().fetchClosedSessions(),
        get().fetchLogs(),
      ]);
    } catch (e) {
      set({ error: toErrorMessage(e) });
      await get().fetchLogs();
    } finally {
      set({ isLoading: false });
    }
  },

  stopEngine: async () => {
    set({ isLoading: true, error: null });
    try {
      await invoke("stop_engine");
      await Promise.all([
        get().fetchStatus(),
        get().fetchProxies(),
        get().fetchTraffic(),
        get().fetchSessions(),
        get().fetchClosedSessions(),
        get().fetchLogs(),
      ]);
    } catch (e) {
      set({ error: toErrorMessage(e) });
      await get().fetchLogs();
    } finally {
      set({ isLoading: false });
    }
  },

  toggleSystemProxy: async () => {
    const { status } = get();
    try {
      if (status.systemProxyEnabled) {
        await invoke("disable_system_proxy");
      } else {
        await invoke("enable_system_proxy");
      }
      await Promise.all([get().fetchStatus(), get().fetchLogs()]);
    } catch (e) {
      set({ error: toErrorMessage(e) });
      await get().fetchLogs();
    }
  },

  selectProxy: async (groupId: string, nodeId: string) => {
    try {
      await invoke("select_proxy", { groupId, nodeId });
      await Promise.all([get().fetchProxies(), get().fetchStatus(), get().fetchLogs()]);
    } catch (e) {
      set({ error: toErrorMessage(e) });
      await get().fetchLogs();
    }
  },

  testProxy: async (nodeId: string) => {
    try {
      const report = await invoke<ProxyTestReport>("test_proxy", { nodeId });
      set({ testReport: report, error: null });
      await Promise.all([get().fetchProxies(), get().fetchLogs()]);
      return report;
    } catch (e) {
      set({ error: toErrorMessage(e) });
      await get().fetchLogs();
      throw e;
    }
  },

  testAllProxies: async (groupId: string) => {
    try {
      const report = await invoke<ProxyTestReport>("test_all_proxies", { groupId });
      set({ testReport: report, error: null });
      await Promise.all([get().fetchProxies(), get().fetchLogs()]);
      return report;
    } catch (e) {
      set({ error: toErrorMessage(e) });
      await get().fetchLogs();
      throw e;
    }
  },

  closeAllSessions: async () => {
    try {
      await invoke("close_all_sessions");
      await Promise.all([get().fetchSessions(), get().fetchClosedSessions(), get().fetchTraffic(), get().fetchLogs()]);
    } catch (e) {
      set({ error: toErrorMessage(e) });
      await get().fetchLogs();
    }
  },

  clearSessionHistory: async () => {
    try {
      await invoke("clear_session_history");
      await Promise.all([get().fetchClosedSessions(), get().fetchLogs()]);
    } catch (e) {
      set({ error: toErrorMessage(e) });
      await get().fetchLogs();
    }
  },

  updateConfig: async (config: Partial<ConfigSettings>) => {
    try {
      const merged = { ...get().config, ...config };
      const saved = await invoke<ConfigSettings>("set_config", { config: merged });
      set({ config: saved, error: null });
      await Promise.all([get().fetchConfig(), get().fetchLogs()]);
    } catch (e) {
      set({ error: toErrorMessage(e) });
      await get().fetchLogs();
    }
  },

  clearError: () => set({ error: null }),
}));
