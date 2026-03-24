import { useEffect } from "react";
import { useAppStore } from "../store/appStore";

export function RulesPage() {
  const { profileSummary, fetchProfileSummary } = useAppStore();

  useEffect(() => {
    void fetchProfileSummary();
  }, [fetchProfileSummary]);

  const summary = profileSummary;

  return (
    <div className="flex flex-col gap-4">
      <div className="grid-3">
        <SummaryCard label="Mode" value={summary.mode.toUpperCase()} caption="Current routing mode" />
        <SummaryCard label="Rules" value={summary.ruleCount.toString()} caption="Loaded rule entries" />
        <SummaryCard label="UDP Proxies" value={summary.udpProxyCount.toString()} caption="Nodes that support UDP" />
      </div>

      <div className="grid-2">
        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">Routing Summary</div>
              <div className="card-subtitle">Selections and current config snapshot</div>
            </div>
          </div>
          <div className="grid-2 gap-4">
            <div>
              <div className="text-sm text-muted">Proxy Groups</div>
              <div style={{ fontWeight: 800, fontSize: 22 }}>{summary.proxyGroupCount}</div>
            </div>
            <div>
              <div className="text-sm text-muted">Log Level</div>
              <div style={{ fontWeight: 800, fontSize: 22 }}>{summary.logLevel}</div>
            </div>
          </div>
          <div style={{ marginTop: 18 }}>
            <div className="text-sm text-muted" style={{ marginBottom: 8 }}>Manual Selections</div>
            {summary.manualSelections.length > 0 ? (
              <div className="flex gap-2" style={{ flexWrap: "wrap" }}>
                {summary.manualSelections.map((item) => (
                  <span key={item} className="badge badge-info" style={{ textTransform: "none" }}>
                    {item}
                  </span>
                ))}
              </div>
            ) : (
              <div className="empty-state" style={{ padding: "24px" }}>
                <div className="empty-state-title">No Manual Selections</div>
                <div className="empty-state-description">
                  Select a node in a selector group to make the routing explicit.
                </div>
              </div>
            )}
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">Rule Preview</div>
              <div className="card-subtitle">First rules from the loaded profile</div>
            </div>
          </div>
          {summary.rulePreview.length > 0 ? (
            <div style={{ display: "grid", gap: 10 }}>
              {summary.rulePreview.map((rule, index) => (
                <div
                  key={`${index}-${rule}`}
                  style={{
                    padding: "12px 14px",
                    borderRadius: 14,
                    border: "1px solid var(--border-color)",
                    background: "var(--bg-soft)",
                  }}
                >
                  <div className="text-sm text-muted">Rule {index + 1}</div>
                  <div style={{ fontWeight: 600, marginTop: 2 }}>{rule}</div>
                </div>
              ))}
            </div>
          ) : (
            <div className="empty-state">
              <RulesIcon />
              <div className="empty-state-title">No Rule Preview</div>
              <div className="empty-state-description">
                Load a subscription or config file to inspect routing rules here.
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function SummaryCard({ label, value, caption }: { label: string; value: string; caption: string }) {
  return (
    <div className="card" style={{ marginBottom: 0 }}>
      <div className="card-header" style={{ marginBottom: 10 }}>
        <div>
          <div className="card-title">{label}</div>
          <div className="card-subtitle">{caption}</div>
        </div>
      </div>
      <div style={{ fontSize: 28, fontWeight: 800, letterSpacing: "-0.04em" }}>{value}</div>
    </div>
  );
}

function RulesIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      style={{ width: 48, height: 48, color: "var(--text-muted)" }}
    >
      <path d="M3 6h18M3 12h18M3 18h18" />
      <circle cx="6" cy="6" r="1.5" fill="currentColor" />
      <circle cx="6" cy="12" r="1.5" fill="currentColor" />
      <circle cx="6" cy="18" r="1.5" fill="currentColor" />
    </svg>
  );
}
