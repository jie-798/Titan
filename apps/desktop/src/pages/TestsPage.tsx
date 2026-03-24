import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "../store/appStore";

type TestScope = "proxy" | "group" | "all";

export function TestsPage() {
  const { proxies, testReport, fetchProxies, testProxy, testAllProxies } = useAppStore();
  const [scope, setScope] = useState<TestScope>("all");
  const [selectedGroupId, setSelectedGroupId] = useState<string>("");
  const [selectedNodeId, setSelectedNodeId] = useState<string>("");
  const [testUrl, setTestUrl] = useState("http://www.gstatic.com/generate_204");
  const [timeoutSecs, setTimeoutSecs] = useState(5);
  const [running, setRunning] = useState(false);

  useEffect(() => {
    void fetchProxies();
  }, [fetchProxies]);

  const selectedGroup = useMemo(
    () => proxies.find((group) => group.id === selectedGroupId) || proxies[0] || null,
    [proxies, selectedGroupId],
  );

  useEffect(() => {
    if (!selectedGroup) {
      setSelectedNodeId("");
      return;
    }
    if (!selectedGroupId) {
      setSelectedGroupId(selectedGroup.id);
    }
    if (!selectedNodeId || !selectedGroup.nodes.some((node) => node.id === selectedNodeId)) {
      setSelectedNodeId(selectedGroup.nodes[0]?.id || "");
    }
  }, [selectedGroup, selectedGroupId, selectedNodeId]);

  const selectedNode = selectedGroup?.nodes.find((node) => node.id === selectedNodeId) || null;

  const runBatchTest = async () => {
    setRunning(true);
    try {
      if (scope === "proxy") {
        if (selectedNodeId) {
          await testProxy(selectedNodeId);
        }
      } else if (scope === "group") {
        if (selectedGroupId) {
          await testAllProxies(selectedGroupId);
        }
      } else {
        if (selectedGroupId) {
          await testAllProxies(selectedGroupId);
        } else if (selectedNodeId) {
          await testProxy(selectedNodeId);
        }
      }
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="grid-2">
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Batch Tester</div>
            <div className="card-subtitle">Measure proxy latency and availability</div>
          </div>
        </div>

        <div className="grid-2 gap-4">
          <div>
            <div className="text-sm text-muted" style={{ marginBottom: 8 }}>Scope</div>
            <div className="flex gap-2" style={{ flexWrap: "wrap" }}>
              <button className={`btn btn-secondary btn-sm ${scope === "proxy" ? "btn-active" : ""}`} onClick={() => setScope("proxy")}>
                Single Proxy
              </button>
              <button className={`btn btn-secondary btn-sm ${scope === "group" ? "btn-active" : ""}`} onClick={() => setScope("group")}>
                Group
              </button>
              <button className={`btn btn-secondary btn-sm ${scope === "all" ? "btn-active" : ""}`} onClick={() => setScope("all")}>
                Best Group
              </button>
            </div>
          </div>
          <div>
            <div className="text-sm text-muted" style={{ marginBottom: 8 }}>Test URL</div>
            <input className="input" value={testUrl} onChange={(event) => setTestUrl(event.target.value)} />
          </div>
        </div>

        <div className="grid-2 gap-4" style={{ marginTop: 16 }}>
          <div>
            <div className="text-sm text-muted" style={{ marginBottom: 8 }}>Timeout (seconds)</div>
            <input
              className="input"
              type="number"
              min={1}
              max={30}
              value={timeoutSecs}
              onChange={(event) => setTimeoutSecs(Number(event.target.value) || 5)}
            />
          </div>
          <div>
            <div className="text-sm text-muted" style={{ marginBottom: 8 }}>Selected Group</div>
            <select
              className="input"
              value={selectedGroupId}
              onChange={(event) => setSelectedGroupId(event.target.value)}
            >
              {proxies.map((group) => (
                <option key={group.id} value={group.id}>
                  {group.name}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div style={{ marginTop: 16 }}>
          <div className="text-sm text-muted" style={{ marginBottom: 8 }}>Selected Proxy</div>
          <select
            className="input"
            value={selectedNodeId}
            onChange={(event) => setSelectedNodeId(event.target.value)}
          >
            {selectedGroup?.nodes.map((node) => (
              <option key={node.id} value={node.id}>
                {node.name} ¡¤ {node.server}:{node.port}
              </option>
            ))}
          </select>
        </div>

        <div className="flex gap-2" style={{ marginTop: 16 }}>
          <button className="btn btn-primary" onClick={() => void runBatchTest()} disabled={running}>
            {running ? "Testing..." : "Run Test"}
          </button>
        </div>

        <div className="grid-2 gap-4" style={{ marginTop: 24 }}>
          <Stat label="Mode" value={testReport?.modeLabel || "Idle"} />
          <Stat label="Scope" value={testReport?.scopeLabel || "No report"} />
          <Stat label="Latency target" value={testReport ? `${testReport.timeoutMs} ms timeout` : "-"} />
          <Stat label="Elapsed" value={testReport ? `${testReport.elapsedMs} ms` : "-"} />
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Test Results</div>
            <div className="card-subtitle">Batch proxy latency report</div>
          </div>
        </div>

        {testReport ? (
          <>
            <div className="grid-2 gap-4" style={{ marginBottom: 16 }}>
              <Stat label="OK" value={String(testReport.ok)} />
              <Stat label="Failed" value={String(testReport.failed)} />
              <Stat label="Skipped" value={String(testReport.skipped)} />
              <Stat label="Targets" value={String(testReport.items.length)} />
            </div>
            <table className="table">
              <thead>
                <tr>
                  <th>Target</th>
                  <th>Resolved</th>
                  <th>Status</th>
                  <th>Latency</th>
                  <th>Message</th>
                </tr>
              </thead>
              <tbody>
                {testReport.items.map((item) => (
                  <tr key={`${item.requestedName}-${item.resolvedName || "na"}`}>
                    <td style={{ fontWeight: 600 }}>{item.requestedName}</td>
                    <td>{item.resolvedName || "-"}</td>
                    <td>
                      <span className={`badge ${statusBadge(item.status)}`}>{item.status}</span>
                    </td>
                    <td>{item.latencyMs !== null ? `${item.latencyMs} ms` : "-"}</td>
                    <td className="text-sm text-muted">{item.message || "-"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        ) : (
          <div className="empty-state">
            <EmptyIcon />
            <div className="empty-state-title">No Test Report</div>
            <div className="empty-state-description">Run a proxy batch test to see resolved node latency and health.</div>
          </div>
        )}
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-sm text-muted">{label}</div>
      <div style={{ fontSize: 22, fontWeight: 800 }}>{value}</div>
    </div>
  );
}

function statusBadge(status: string): string {
  if (status === "ok") return "badge-success";
  if (status === "failed") return "badge-error";
  return "badge-neutral";
}

function EmptyIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ width: 48, height: 48, color: "var(--text-muted)" }}>
      <path d="M4 6h16v12H4z" />
      <path d="M8 10h8M8 14h5" />
    </svg>
  );
}
