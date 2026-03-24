import { useEffect, useState } from "react";
import { NetworkPage } from "./pages/NetworkPage";
import { OverviewPage } from "./pages/OverviewPage";
import { ProxiesPage } from "./pages/ProxiesPage";
import { RulesPage } from "./pages/RulesPage";
import { SettingsPage } from "./pages/SettingsPage";
import { TestsPage } from "./pages/TestsPage";
import { UpdatesPage } from "./pages/UpdatesPage";
import { useAppStore } from "./store/appStore";

type PageKey = "overview" | "proxies" | "network" | "tests" | "rules" | "updates" | "settings";

const pages: Record<PageKey, { label: string; subtitle: string }> = {
  overview: { label: "Overview", subtitle: "Status, control, and quick actions" },
  proxies: { label: "Proxies", subtitle: "Groups, nodes, and selection" },
  network: { label: "Connections", subtitle: "Live sessions, traffic, and recent routes" },
  tests: { label: "Tests", subtitle: "Batch latency tests and proxy health results" },
  rules: { label: "Rules", subtitle: "Routing policy summary and selection state" },
  updates: { label: "Updates", subtitle: "Subscription usage and data source refresh" },
  settings: { label: "Settings", subtitle: "Runtime, proxy, and startup behavior" },
};

const navItems: Array<{ key: PageKey; label: string }> = [
  { key: "overview", label: "Overview" },
  { key: "proxies", label: "Proxies" },
  { key: "network", label: "Connections" },
  { key: "tests", label: "Tests" },
  { key: "rules", label: "Rules" },
  { key: "updates", label: "Updates" },
  { key: "settings", label: "Settings" },
];

export default function App() {
  const [page, setPage] = useState<PageKey>("overview");
  const { status, error, clearError, fetchStatus, fetchConfig, fetchProfileSummary, profileSummary } = useAppStore();

  useEffect(() => {
    void Promise.all([fetchStatus(), fetchConfig(), fetchProfileSummary()]);
  }, [fetchConfig, fetchProfileSummary, fetchStatus]);

  const currentPage = pages[page];
  const activeNodeLabel = status.activeNode || "No active node";
  const profileLabel =
    profileSummary.subscription?.usagePercent !== null && profileSummary.subscription?.usagePercent !== undefined
      ? `Subscription ${profileSummary.subscription.usagePercent.toFixed(1)}% used`
      : `${profileSummary.proxyCount} proxies ¡¤ ${profileSummary.proxyGroupCount} groups`;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">T</div>
          <div>
            <div className="brand-title">Titan</div>
            <div className="brand-subtitle">Desktop Control Surface</div>
          </div>
        </div>

        <nav className="nav">
          {navItems.map((item) => (
            <button
              key={item.key}
              className={`nav-item ${page === item.key ? "active" : ""}`}
              onClick={() => setPage(item.key)}
            >
              {item.label}
            </button>
          ))}
        </nav>

        <div className="sidebar-panel">
          <div className="sidebar-label">Runtime</div>
          <div className="sidebar-value">{status.engineRunning ? "Running" : "Stopped"}</div>
          <div className="sidebar-metadata">
            Mixed {status.inboundBind}:{status.inboundPort}
          </div>
          <div className="sidebar-metadata" style={{ marginTop: 8 }}>
            {activeNodeLabel}
          </div>
          <div className="sidebar-metadata">{profileLabel}</div>
        </div>
      </aside>

      <main className="main-panel">
        <header className="topbar">
          <div>
            <div className="page-title">{currentPage.label}</div>
            <div className="page-subtitle">{currentPage.subtitle}</div>
          </div>
          <div className="topbar-status">
            <span className={`status-dot ${status.engineRunning ? "online" : "offline"}`} />
            {status.systemProxyEnabled ? "Windows proxy enabled" : "Windows proxy disabled"}
          </div>
        </header>

        <section className="content">
          {error ? (
            <div className="global-error-banner">
              <div>
                <div className="global-error-title">Action failed</div>
                <div className="global-error-message">{error}</div>
              </div>
              <button className="btn btn-secondary btn-sm" onClick={clearError}>
                Dismiss
              </button>
            </div>
          ) : null}

          {page === "overview" ? <OverviewPage /> : null}
          {page === "proxies" ? <ProxiesPage /> : null}
          {page === "network" ? <NetworkPage /> : null}
          {page === "tests" ? <TestsPage /> : null}
          {page === "rules" ? <RulesPage /> : null}
          {page === "updates" ? <UpdatesPage /> : null}
          {page === "settings" ? <SettingsPage /> : null}
        </section>
      </main>
    </div>
  );
}
