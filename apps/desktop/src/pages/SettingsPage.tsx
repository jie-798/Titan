import { useEffect, useState } from "react";
import { useAppStore } from "../store/appStore";

export function SettingsPage() {
  const { config, logs, fetchConfig, fetchLogs, updateConfig, status } = useAppStore();

  const [localConfig, setLocalConfig] = useState(config);
  const [hasChanges, setHasChanges] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    void Promise.all([fetchConfig(), fetchLogs()]);
  }, [fetchConfig, fetchLogs]);

  useEffect(() => {
    setLocalConfig(config);
  }, [config]);

  const handleChange = (key: keyof typeof config, value: string | number | boolean) => {
    setLocalConfig((prev) => ({ ...prev, [key]: value }));
    setHasChanges(true);
  };

  const handleSave = async () => {
    setIsSaving(true);
    await updateConfig(localConfig);
    setHasChanges(false);
    setIsSaving(false);
  };

  const handleReset = () => {
    setLocalConfig(config);
    setHasChanges(false);
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="card">
        <div className="card-header">
          <div className="card-title">General Settings</div>
        </div>
        <div className="flex flex-col gap-4">
          <div>
            <label className="text-sm text-muted" style={{ display: "block", marginBottom: 4 }}>
              Config File Path
            </label>
            <input
              type="text"
              className="input"
              value={localConfig.configPath}
              onChange={(e) => handleChange("configPath", e.target.value)}
            />
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <div className="card-title">Proxy Engine</div>
        </div>
        <div className="grid-2 gap-4">
          <div>
            <label className="text-sm text-muted" style={{ display: "block", marginBottom: 4 }}>
              Inbound Bind Address
            </label>
            <input
              type="text"
              className="input"
              value={localConfig.inboundBind}
              onChange={(e) => handleChange("inboundBind", e.target.value)}
            />
          </div>
          <div>
            <label className="text-sm text-muted" style={{ display: "block", marginBottom: 4 }}>
              Inbound Port
            </label>
            <input
              type="number"
              className="input"
              value={localConfig.inboundPort}
              onChange={(e) => handleChange("inboundPort", parseInt(e.target.value, 10) || 0)}
            />
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <div className="card-title">API Server</div>
        </div>
        <div className="grid-2 gap-4">
          <div>
            <label className="text-sm text-muted" style={{ display: "block", marginBottom: 4 }}>
              API Bind Address
            </label>
            <input
              type="text"
              className="input"
              value={localConfig.apiBind}
              onChange={(e) => handleChange("apiBind", e.target.value)}
            />
          </div>
          <div>
            <label className="text-sm text-muted" style={{ display: "block", marginBottom: 4 }}>
              API Port
            </label>
            <input
              type="number"
              className="input"
              value={localConfig.apiPort}
              onChange={(e) => handleChange("apiPort", parseInt(e.target.value, 10) || 0)}
            />
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <div className="card-title">System Proxy</div>
        </div>
        <div className="flex flex-col gap-4">
          <div className="flex items-center justify-between">
            <div>
              <div style={{ fontWeight: 500 }}>Auto-enable on Launch</div>
              <div className="text-sm text-muted">
                Automatically enable system proxy when Titan starts
              </div>
            </div>
            <button
              className={`toggle ${localConfig.autoEnableSystemProxy ? "active" : ""}`}
              onClick={() => handleChange("autoEnableSystemProxy", !localConfig.autoEnableSystemProxy)}
            />
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <div className="card-title">Startup</div>
        </div>
        <div className="flex flex-col gap-4">
          <div className="flex items-center justify-between">
            <div>
              <div style={{ fontWeight: 500 }}>Auto-start Engine</div>
              <div className="text-sm text-muted">
                Automatically start the proxy engine when the app launches
              </div>
            </div>
            <button
              className={`toggle ${localConfig.autoStartEngine ? "active" : ""}`}
              onClick={() => handleChange("autoStartEngine", !localConfig.autoStartEngine)}
            />
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <div className="card-title">Current Runtime Info</div>
        </div>
        <div className="grid-3 gap-4">
          <div>
            <div className="text-sm text-muted">Engine Status</div>
            <div style={{ fontWeight: 500 }}>
              <span className={`badge ${status.engineRunning ? "badge-success" : "badge-neutral"}`}>
                {status.engineRunning ? "Running" : "Stopped"}
              </span>
            </div>
          </div>
          <div>
            <div className="text-sm text-muted">System Proxy</div>
            <div style={{ fontWeight: 500 }}>
              <span className={`badge ${status.systemProxyEnabled ? "badge-success" : "badge-neutral"}`}>
                {status.systemProxyEnabled ? "Enabled" : "Disabled"}
              </span>
            </div>
          </div>
          <div>
            <div className="text-sm text-muted">Inbound</div>
            <div style={{ fontWeight: 500 }}>
              {status.inboundBind || "127.0.0.1"}:{status.inboundPort}
            </div>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Diagnostics</div>
            <div className="card-subtitle">Recent desktop actions and failures</div>
          </div>
          <button className="btn btn-secondary btn-sm" onClick={() => void fetchLogs()}>
            Refresh Logs
          </button>
        </div>
        <div className="log-viewer">
          {logs.length > 0 ? (
            logs.map((line, index) => (
              <div key={`${index}-${line}`} className="log-line">
                {line}
              </div>
            ))
          ) : (
            <div className="text-sm text-muted">No logs yet.</div>
          )}
        </div>
      </div>

      <div className="flex justify-end gap-2">
        <button
          className="btn btn-secondary"
          onClick={handleReset}
          disabled={!hasChanges || isSaving}
        >
          Reset
        </button>
        <button
          className="btn btn-primary"
          onClick={() => void handleSave()}
          disabled={!hasChanges || isSaving}
        >
          {isSaving ? "Saving..." : "Save Changes"}
        </button>
      </div>
    </div>
  );
}
