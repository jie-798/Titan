import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../store/appStore";

export function UpdatesPage() {
  const { profileSummary, fetchProfileSummary, config } = useAppStore();
  const [isRefreshing, setIsRefreshing] = useState(false);

  useEffect(() => {
    void fetchProfileSummary();
  }, [fetchProfileSummary]);

  const subscription = profileSummary.subscription;

  const handleRefreshSubscription = async () => {
    setIsRefreshing(true);
    try {
      await invoke("refresh_subscription");
      await fetchProfileSummary();
    } finally {
      setIsRefreshing(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="grid-2">
        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">Subscription Usage</div>
              <div className="card-subtitle">Saved beside your config as a metadata sidecar</div>
            </div>
            <button className="btn btn-secondary btn-sm" onClick={() => void handleRefreshSubscription()} disabled={isRefreshing}>
              <UpdateIcon />
              {isRefreshing ? "Refreshing..." : "Refresh"}
            </button>
          </div>
          {subscription ? (
            <div className="flex flex-col gap-3">
              <UsageRow label="Used" value={formatBytes(subscription.used)} />
              <UsageRow label="Remaining" value={formatBytes(subscription.remaining)} />
              <UsageRow label="Total" value={formatBytes(subscription.total)} />
              <UsageRow label="Usage" value={subscription.usagePercent === null ? "Unknown" : `${subscription.usagePercent.toFixed(2)}%`} />
              <UsageRow label="Fetched" value={subscription.fetchedAtLocal || "Unknown"} />
              <UsageRow label="Expire" value={subscription.expireLocal || "Unknown"} />
              <UsageRow label="Source" value={subscription.source || "Unknown"} />
            </div>
          ) : (
            <div className="empty-state">
              <SubscriptionsIcon />
              <div className="empty-state-title">No Subscription Metadata</div>
              <div className="empty-state-description">
                Import a subscription first to see usage, expiry, and refresh information here.
              </div>
            </div>
          )}
        </div>

        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">Data Sources</div>
              <div className="card-subtitle">Static data refresh tools are available in CLI and runtime</div>
            </div>
          </div>
          <div className="grid-2 gap-4">
            <SummaryCard label="Config File" value={config.configPath} caption="Main subscription/profile file" />
            <SummaryCard label="Profile Rules" value={`${profileSummary.ruleCount}`} caption="Loaded rule entries" />
          </div>
          <div style={{ marginTop: 16 }}>
            <div className="text-sm text-muted" style={{ marginBottom: 8 }}>What is supported</div>
            <div style={{ display: "grid", gap: 10 }}>
              <FeatureItem title="Subscription auto-refresh" description="The runtime can refresh the subscription on a timer and reload the engine." />
              <FeatureItem title="GEOIP / GEOSITE updates" description="Database updates are supported through the CLI workflow and are ready to be surfaced here next." />
              <FeatureItem title="System config persistence" description="Desktop settings are stored separately from the subscription config so the profile stays clean." />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function UsageRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between" style={{ padding: "10px 12px", borderRadius: 14, background: "var(--bg-soft)", border: "1px solid var(--border-color)" }}>
      <div className="text-sm text-muted">{label}</div>
      <div style={{ fontWeight: 700 }}>{value}</div>
    </div>
  );
}

function SummaryCard({ label, value, caption }: { label: string; value: string; caption: string }) {
  return (
    <div style={{ padding: 14, borderRadius: 16, border: "1px solid var(--border-color)", background: "var(--bg-soft)" }}>
      <div className="text-sm text-muted">{label}</div>
      <div style={{ fontSize: 18, fontWeight: 800, marginTop: 4 }}>{value}</div>
      <div className="text-sm text-muted" style={{ marginTop: 2 }}>{caption}</div>
    </div>
  );
}

function FeatureItem({ title, description }: { title: string; description: string }) {
  return (
    <div style={{ padding: "12px 14px", borderRadius: 16, border: "1px solid var(--border-color)", background: "white" }}>
      <div style={{ fontWeight: 700 }}>{title}</div>
      <div className="text-sm text-muted" style={{ marginTop: 4 }}>{description}</div>
    </div>
  );
}

function formatBytes(value: number | null): string {
  if (value === null) return "Unknown";
  if (value === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  return `${(value / Math.pow(1024, index)).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function UpdateIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 16, height: 16 }}>
      <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
      <path d="M3 3v5h5" />
      <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" />
      <path d="M16 16h5v5" />
    </svg>
  );
}

function SubscriptionsIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      style={{ width: 48, height: 48, color: "var(--text-muted)" }}
    >
      <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z" />
      <path d="M22 6l-10 7L2 6" />
    </svg>
  );
}
