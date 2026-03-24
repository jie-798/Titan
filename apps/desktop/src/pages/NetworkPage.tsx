import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "../store/appStore";

type SessionTab = "active" | "closed";

export function NetworkPage() {
  const {
    traffic,
    sessions,
    closedSessions,
    fetchTraffic,
    fetchSessions,
    fetchClosedSessions,
    closeAllSessions,
    clearSessionHistory,
  } = useAppStore();
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState<SessionTab>("active");

  useEffect(() => {
    let disposed = false;
    let timer: number | undefined;

    const refresh = async () => {
      await Promise.all([fetchTraffic(), fetchSessions(), fetchClosedSessions()]);
      if (!disposed) {
        timer = window.setTimeout(() => {
          void refresh();
        }, 2500);
      }
    };

    void refresh();

    return () => {
      disposed = true;
      if (timer !== undefined) {
        window.clearTimeout(timer);
      }
    };
  }, [fetchClosedSessions, fetchSessions, fetchTraffic]);

  const sessionList = tab === "active" ? sessions : closedSessions;

  const filteredSessions = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) {
      return sessionList;
    }
    return sessionList.filter((session) => {
      const haystack = [
        session.sourceIp,
        String(session.sourcePort),
        session.destinationHost,
        String(session.destinationPort),
        session.proxyNodeId || "",
        session.ruleId || "",
        session.status,
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(normalized);
    });
  }, [query, sessionList]);

  return (
    <div className="flex flex-col gap-4">
      <div className="grid-2">
        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">Traffic Overview</div>
              <div className="card-subtitle">Live throughput and session count</div>
            </div>
          </div>
          <div className="grid-2 gap-4">
            <Metric label="Download" value={formatSpeed(traffic.downloadBytesPerSecond)} tone="var(--success)" />
            <Metric label="Upload" value={formatSpeed(traffic.uploadBytesPerSecond)} tone="var(--accent)" />
          </div>
          <div className="grid-2 gap-4" style={{ marginTop: 18 }}>
            <div>
              <div className="text-sm text-muted">Total Download</div>
              <div style={{ fontWeight: 700 }}>{formatBytes(traffic.totalDownloadBytes)}</div>
            </div>
            <div>
              <div className="text-sm text-muted">Total Upload</div>
              <div style={{ fontWeight: 700 }}>{formatBytes(traffic.totalUploadBytes)}</div>
            </div>
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">Connections</div>
              <div className="card-subtitle">Active sessions observed by Titan</div>
            </div>
          </div>
          <div style={{ fontSize: "48px", fontWeight: 800, letterSpacing: "-0.04em" }}>
            {traffic.activeConnections}
          </div>
          <div className="text-sm text-muted">Active network sessions</div>
          <div className="flex gap-2" style={{ marginTop: 12 }}>
            <span className="badge badge-info">Live</span>
            <span className="badge badge-neutral">Auto refresh</span>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-header" style={{ alignItems: "flex-start", gap: 12 }}>
          <div>
            <div className="card-title">Session History</div>
            <div className="card-subtitle">Inspect active and closed connections</div>
          </div>
          <div className="flex gap-2" style={{ flexWrap: "wrap", justifyContent: "flex-end" }}>
            <button
              className={`btn btn-secondary btn-sm ${tab === "active" ? "btn-active" : ""}`}
              onClick={() => setTab("active")}
            >
              Active {sessions.length}
            </button>
            <button
              className={`btn btn-secondary btn-sm ${tab === "closed" ? "btn-active" : ""}`}
              onClick={() => setTab("closed")}
            >
              Closed {closedSessions.length}
            </button>
            <button className="btn btn-secondary btn-sm" onClick={() => void closeAllSessions()}>
              Close All
            </button>
            <button className="btn btn-secondary btn-sm" onClick={() => void clearSessionHistory()}>
              Clear History
            </button>
            <input
              className="input"
              style={{ maxWidth: 340 }}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Filter by host, proxy, IP, or rule"
            />
          </div>
        </div>

        {filteredSessions.length > 0 ? (
          <table className="table">
            <thead>
              <tr>
                <th>Source</th>
                <th>Destination</th>
                <th>Proxy</th>
                <th>Traffic</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {filteredSessions.map((session) => (
                <tr key={session.id}>
                  <td>
                    <div style={{ fontWeight: 600 }}>{session.sourceIp}</div>
                    <div className="text-sm text-muted">Port {session.sourcePort}</div>
                  </td>
                  <td>
                    <div style={{ fontWeight: 600 }}>{session.destinationHost}</div>
                    <div className="text-sm text-muted">:{session.destinationPort}</div>
                  </td>
                  <td>
                    <div style={{ fontWeight: 600 }}>{session.proxyNodeId || "DIRECT"}</div>
                    <div className="text-sm text-muted">{session.ruleId || "No rule"}</div>
                  </td>
                  <td>
                    <div className="text-sm text-muted">Down {formatBytes(session.downloadBytes)}</div>
                    <div className="text-sm text-muted">Up {formatBytes(session.uploadBytes)}</div>
                    <div className="text-sm text-muted">Age {session.startTime}s</div>
                  </td>
                  <td>
                    <span className={`badge ${session.status === "active" ? "badge-success" : "badge-neutral"}`}>
                      {session.status}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <div className="empty-state">
            <NetworkIcon />
            <div className="empty-state-title">No Sessions</div>
            <div className="empty-state-description">
              {tab === "active"
                ? "Active sessions will appear here when the proxy engine is forwarding traffic."
                : "Closed session history is empty. It will accumulate as connections end."}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function Metric({ label, value, tone }: { label: string; value: string; tone: string }) {
  return (
    <div>
      <div className="text-sm text-muted">{label}</div>
      <div style={{ fontSize: 24, fontWeight: 800, color: tone }}>{value}</div>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / Math.pow(1024, index)).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

function NetworkIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      style={{ width: 48, height: 48, color: "var(--text-muted)" }}
    >
      <path d="M5 12.55a11 11 0 0 1 14.08 0M1.42 9a16 16 0 0 1 21.16 0M8.53 16.11a6 6 0 0 1 6.95 0M12 20h.01" />
    </svg>
  );
}
