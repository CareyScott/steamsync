import { useState } from "react";
import { Alert, Button, Result, Space, Spin, Typography } from "antd";
import { applyChanges } from "../api";
import type { ApplyResult, SyncOptions } from "../types";

const { Paragraph } = Typography;

interface Props {
  options: SyncOptions;
  selectedAppNames: string[];
}

export default function ApplyView({ options, selectedAppNames }: Props) {
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<ApplyResult | null>(null);

  const handleApply = async () => {
    setRunning(true);
    setResult(null);
    try {
      const r = await applyChanges(options, selectedAppNames);
      setResult(r);
    } catch (e) {
      setResult({ error: String(e) });
    } finally {
      setRunning(false);
    }
  };

  if (running) {
    return (
      <div style={{ textAlign: "center", padding: 60 }}>
        <Spin size="large" />
        <Paragraph style={{ marginTop: 16 }}>
          Writing shortcuts.vdf and downloading art…
        </Paragraph>
      </div>
    );
  }

  if (result?.error) {
    return (
      <Space direction="vertical" style={{ width: "100%" }}>
        <Alert
          type="error"
          message="Apply failed"
          description={result.error}
          showIcon
        />
        <Button onClick={handleApply}>Retry</Button>
      </Space>
    );
  }

  if (result) {
    return (
      <Result
        status="success"
        title="Done"
        subTitle={`Added ${result.added ?? 0}, removed ${result.removed ?? 0} for ${result.username ?? ""} (${result.steamid ?? ""}).`}
        extra={[
          <Paragraph key="restart">Restart Steam to see the new shortcuts.</Paragraph>,
        ]}
      />
    );
  }

  return (
    <Space direction="vertical">
      <Paragraph>
        Ready to write {selectedAppNames.length} game(s) to{" "}
        <code>shortcuts.vdf</code> for SteamID{" "}
        <strong>{options.steamid}</strong>.
      </Paragraph>
      <Button type="primary" size="large" onClick={handleApply}>
        Write shortcuts
      </Button>
    </Space>
  );
}
