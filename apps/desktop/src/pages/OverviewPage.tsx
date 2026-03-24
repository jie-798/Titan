import { useEffect } from "react";
import { useAppStore } from "../store/appStore";

export function OverviewPage() {
  const {
    status,
    traffic,
    sessions,
    proxies,
    isLoading,
    fetchStatus,
    fetchTraffic,
    fetchSessions,
    fetchProxies,
    startEngine,
    stopEngine,
    toggleSystemProxy,
  } = useAppStore();

  useEffect(() => {
    let disposed = false;
    let inFlight = false;
    let timer: number | undefined;

    const loadInitial = async () => {
      await Promise.all([
        fetchStatus(),
        fetchProxies(),
        fetchTraffic(),
        fetchSessions(),
      ]);
    };

    const tick = async () => {
      if (disposed || inFlight) {
        return;
      }

      inFlight = true;
      try {
        await fetchStatus();
        if (useAppStore.getState().status.engineRunning) {
          await fetchTraffic();
          await fetchSessions();
        }
      } finally {
        inFlight = false;
        if (!disposed) {
          timer = window.setTimeout(() => {
            void tick();
          }, 2000);
        }
      }
    };

    void loadInitial().finally(() => {
      if (!disposed) {
        timer = window.setTimeout(() => {
          void tick();
        }, 2000);
      }
    });

    return () => {
      disposed = true;
      if (timer !== undefined) {
        window.clearTimeout(timer);
      }
    };
  }, [fetchProxies, fetchSessions, fetchStatus, fetchTraffic]);

  const handleEngineToggle = () => {
    if (status.engineRunning) {
      void stopEngine();
    } else {
      void startEngine();
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  };

  const formatSpeed = (bytesPerSecond: number): string => {
    return `${formatBytes(bytesPerSecond)}/s`;
  };

  const activeNode = proxies.find((g) =>
    g.nodes.some((n) => n.id === status.activeNode)
  );

  return (
    <div className="flex flex-col gap-4">
      <div className="grid-2">
        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">Engine Status</div>
              <div className="card-subtitle">
                {status.inboundPort > 0
                  ? `Listening on ${status.inboundBind || "127.0.0.1"}:${
                      status.inboundPort
                    }`
                  : "Not configured"}
              </div>
            </div>
            <span
              className={`badge ${
                status.engineRunning ? "badge-success" : "badge-neutral"
              }`}
            >
              {status.engineRunning ? "Running" : "Stopped"}
            </span>
          </div>
          <div className="flex gap-2">
            <button
              className={`btn ${status.engineRunning ? "btn-danger" : "btn-success"}`}
              onClick={handleEngineToggle}
              disabled={isLoading}
            >
              {status.engineRunning ? (
                <>
                  <StopIcon />
                  Stop
                </>
              ) : (
                <>
                  <PlayIcon />
                  Start
                </>
              )}
            </button>
            {status.engineRunning && (
              <button className="btn btn-secondary" onClick={() => void fetchStatus()}>
                <RefreshIcon />
                Refresh
              </button>
            )}
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">System Proxy</div>
              <div className="card-subtitle">
                {status.systemProxyEnabled
                  ? "Windows proxy is enabled"
                  : "Windows proxy is disabled"}
              </div>
            </div>
            <button
              className={`toggle ${status.systemProxyEnabled ? "active" : ""}`}
              onClick={() => void toggleSystemProxy()}
              disabled={!status.engineRunning}
            />
          </div>
          <p className="text-sm text-muted">
            When enabled, system traffic will route through Titan.
          </p>
        </div>
      </div>

      <div className="grid-2">
        <div className="card">
          <div className="card-header">
            <div className="card-title">Traffic</div>
          </div>
          <div className="grid-2 gap-4">
            <div>
              <div className="text-sm text-muted">Download</div>
              <div style={{ fontSize: "20px", fontWeight: 600, color: "var(--success)" }}>
                {formatSpeed(traffic.downloadBytesPerSecond)}
              </div>
              <div className="text-sm text-muted">
                Total: {formatBytes(traffic.totalDownloadBytes)}
              </div>
            </div>
            <div>
              <div className="text-sm text-muted">Upload</div>
              <div style={{ fontSize: "20px", fontWeight: 600, color: "var(--accent)" }}>
                {formatSpeed(traffic.uploadBytesPerSecond)}
              </div>
              <div className="text-sm text-muted">
                Total: {formatBytes(traffic.totalUploadBytes)}
              </div>
            </div>
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <div className="card-title">Connections</div>
          </div>
          <div style={{ fontSize: "36px", fontWeight: 600 }}>
            {traffic.activeConnections}
          </div>
          <div className="text-sm text-muted">Active connections</div>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <div className="card-title">Active Proxy</div>
        </div>
        {activeNode ? (
          <div className="flex items-center gap-3">
            <span
              className={`protocol-badge ${activeNode.nodes[0]?.protocol.toLowerCase()}`}
            >
              {activeNode.nodes[0]?.protocol.charAt(0).toUpperCase()}
            </span>
            <div>
              <div style={{ fontWeight: 500 }}>{activeNode.name}</div>
              <div className="text-sm text-muted">
                {activeNode.nodes[0]?.server}:{activeNode.nodes[0]?.port}
              </div>
            </div>
          </div>
        ) : (
          <div className="empty-state" style={{ padding: "24px" }}>
            <div className="empty-state-title">No Active Proxy</div>
            <div className="empty-state-description">
              Start the engine and select a proxy node to begin
            </div>
          </div>
        )}
      </div>

      <div className="card">
        <div className="card-header">
          <div className="card-title">Recent Sessions</div>
          <span className="badge badge-info">{sessions.length}</span>
        </div>
        {sessions.length > 0 ? (
          <table className="table">
            <thead>
              <tr>
                <th>Source</th>
                <th>Destination</th>
                <th>Proxy</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {sessions.slice(0, 5).map((session) => (
                <tr key={session.id}>
                  <td>
                    {session.sourceIp}:{session.sourcePort}
                  </td>
                  <td>
                    {session.destinationHost}:{session.destinationPort}
                  </td>
                  <td>{session.proxyNodeId || "-"}</td>
                  <td>
                    <span
                      className={`badge ${
                        session.status === "active"
                          ? "badge-success"
                          : "badge-neutral"
                      }`}
                    >
                      {session.status}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <div className="empty-state" style={{ padding: "24px" }}>
            <div className="empty-state-title">No Active Sessions</div>
            <div className="empty-state-description">
              Sessions will appear here when connections are made through the
              proxy
            </div>
          </div>
        )}
      </div>

      {status.error && (
        <div
          className="card"
          style={{ borderColor: "var(--error)", background: "var(--error-muted)" }}
        >
          <div className="card-title" style={{ color: "var(--error)" }}>
            Error
          </div>
          <div className="text-sm">{status.error}</div>
        </div>
      )}
    </div>
  );
}

function PlayIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="currentColor"
      style={{ width: 16, height: 16 }}
    >
      <polygon points="5,3 19,12 5,21" />
    </svg>
  );
}

function StopIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="currentColor"
      style={{ width: 16, height: 16 }}
    >
      <rect x="4" y="4" width="16" height="16" rx="2" />
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
      style={{ width: 16, height: 16 }}
    >
      <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
      <path d="M3 3v5h5" />
      <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" />
      <path d="M16 16h5v5" />
    </svg>
  );
}
