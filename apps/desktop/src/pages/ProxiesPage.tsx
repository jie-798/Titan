import { useEffect, useState } from "react";
import { useAppStore } from "../store/appStore";

export function ProxiesPage() {
  const {
    proxies,
    status,
    fetchProxies,
    fetchStatus,
    selectProxy,
    testProxy,
    testAllProxies,
  } = useAppStore();

  const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);
  const [testingNodeId, setTestingNodeId] = useState<string | null>(null);

  useEffect(() => {
    fetchProxies();
    fetchStatus();
  }, []);

  const selectedGroup = proxies.find((g) => g.id === selectedGroupId) || proxies[0];
  const groupTypeLabels: Record<string, string> = {
    selector: "Manual",
    "url-test": "Auto (URL Test)",
    fallback: "Auto (Fallback)",
  };

  const handleSelectNode = async (nodeId: string) => {
    if (selectedGroup && selectedGroup.type === "selector") {
      await selectProxy(selectedGroup.id, nodeId);
    }
  };

  const handleTestNode = async (nodeId: string) => {
    setTestingNodeId(nodeId);
    await testProxy(nodeId);
    setTestingNodeId(null);
  };

  const handleTestAll = async () => {
    if (selectedGroup) {
      await testAllProxies(selectedGroup.id);
    }
  };

  const getLatencyColor = (latency: number | null): string => {
    if (latency === null) return "var(--text-muted)";
    if (latency < 100) return "var(--success)";
    if (latency < 300) return "var(--warning)";
    return "var(--error)";
  };

  const getProtocolIcon = (protocol: string): string => {
    return protocol.charAt(0).toUpperCase();
  };

  return (
    <div className="flex gap-4" style={{ height: "100%" }}>
      <div
        className="card"
        style={{
          width: "240px",
          flexShrink: 0,
          display: "flex",
          flexDirection: "column",
          marginBottom: 0,
        }}
      >
        <div className="card-header">
          <div className="card-title">Proxy Groups</div>
        </div>
        <div style={{ flex: 1, overflowY: "auto" }}>
          {proxies.map((group) => (
            <div
              key={group.id}
              className={`nav-item ${selectedGroup?.id === group.id ? "active" : ""}`}
              style={{
                margin: "2px 0",
                width: "100%",
                borderRadius: "var(--radius-md)",
              }}
              onClick={() => setSelectedGroupId(group.id)}
            >
              <div style={{ fontWeight: 500, marginBottom: 2 }}>
                {group.name}
              </div>
              <div className="text-sm text-muted">
                {groupTypeLabels[group.type] || group.type} • {group.nodes.length} nodes
              </div>
            </div>
          ))}
          {proxies.length === 0 && (
            <div className="empty-state" style={{ padding: "24px" }}>
              <div className="empty-state-title">No Groups</div>
              <div className="empty-state-description">
                Import a subscription to load proxy groups
              </div>
            </div>
          )}
        </div>
      </div>

      <div
        className="card"
        style={{ flex: 1, display: "flex", flexDirection: "column", marginBottom: 0 }}
      >
        {selectedGroup ? (
          <>
            <div className="card-header">
              <div>
                <div className="card-title">{selectedGroup.name}</div>
                <div className="card-subtitle">
                  {groupTypeLabels[selectedGroup.type]}{" "}
                  {selectedGroup.type === "selector" && selectedGroup.currentNodeId && (
                    <span>• Current: {selectedGroup.nodes.find(n => n.id === selectedGroup.currentNodeId)?.name}</span>
                  )}
                  {selectedGroup.type !== "selector" && selectedGroup.currentNodeId && (
                    <span>• Active: {selectedGroup.nodes.find(n => n.id === selectedGroup.currentNodeId)?.name}</span>
                  )}
                </div>
              </div>
              <div className="flex gap-2">
                {selectedGroup.type !== "selector" && (
                  <button className="btn btn-secondary btn-sm" onClick={handleTestAll}>
                    <RefreshIcon />
                    Test All
                  </button>
                )}
              </div>
            </div>

            <div style={{ flex: 1, overflowY: "auto" }}>
              <table className="table">
                <thead>
                  <tr>
                    <th style={{ width: "40px" }}></th>
                    <th>Name</th>
                    <th>Server</th>
                    <th>Latency</th>
                    <th>Status</th>
                    <th style={{ width: "120px" }}>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {selectedGroup.nodes.map((node) => (
                    <tr
                      key={node.id}
                      style={{
                        background:
                          node.id === selectedGroup.currentNodeId
                            ? "var(--accent-muted)"
                            : undefined,
                      }}
                    >
                      <td>
                        <span
                          className={`protocol-badge ${node.protocol.toLowerCase()}`}
                        >
                          {getProtocolIcon(node.protocol)}
                        </span>
                      </td>
                      <td>
                        <div style={{ fontWeight: 500 }}>{node.name}</div>
                        <div className="text-sm text-muted">{node.protocol}</div>
                      </td>
                      <td>
                        {node.server}:{node.port}
                      </td>
                      <td style={{ color: getLatencyColor(node.latency) }}>
                        {testingNodeId === node.id ? (
                          <span className="text-muted">Testing...</span>
                        ) : node.latency !== null ? (
                          `${node.latency} ms`
                        ) : (
                          <span className="text-muted">-</span>
                        )}
                      </td>
                      <td>
                        <span
                          className={`badge ${
                            node.status === "active"
                              ? "badge-success"
                              : node.status === "error"
                              ? "badge-error"
                              : "badge-neutral"
                          }`}
                        >
                          {node.status}
                        </span>
                      </td>
                      <td>
                        <div className="flex gap-2">
                          {selectedGroup.type === "selector" && (
                            <button
                              className="btn btn-primary btn-sm"
                              onClick={() => handleSelectNode(node.id)}
                              disabled={!status.engineRunning}
                            >
                              Select
                            </button>
                          )}
                          <button
                            className="btn btn-secondary btn-sm"
                            onClick={() => handleTestNode(node.id)}
                            disabled={testingNodeId !== null}
                          >
                            <RefreshIcon />
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </>
        ) : (
          <div className="empty-state">
            <ProxiesEmptyIcon />
            <div className="empty-state-title">No Proxy Group Selected</div>
            <div className="empty-state-description">
              Select a proxy group from the left panel to view its nodes
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function ProxiesEmptyIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      style={{ width: 48, height: 48, color: "var(--text-muted)" }}
    >
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2v4m0 12v4M2 12h4m12 0h4" />
      <path d="M4.93 4.93l2.83 2.83m8.48 8.48l2.83 2.83M4.93 19.07l2.83-2.83m8.48-8.48l2.83-2.83" />
    </svg>
  );
}

function RefreshIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      style={{ width: 14, height: 14 }}
    >
      <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
      <path d="M3 3v5h5" />
      <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" />
      <path d="M16 16h5v5" />
    </svg>
  );
}