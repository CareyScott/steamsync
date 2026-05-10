import { useEffect, useRef, useState } from "react";
import {
  Alert,
  Button,
  Card,
  Modal,
  Progress,
  Result,
  Space,
  Statistic,
  Typography,
} from "antd";
import {
  CheckCircleOutlined,
  ExclamationCircleOutlined,
  PictureOutlined,
  SaveOutlined,
} from "@ant-design/icons";
import { applyChanges, onApplyProgress, type ApplyEvent } from "../api";
import type { ApplyResult, SyncOptions } from "../types";

const { Paragraph, Text } = Typography;

interface Props {
  options: SyncOptions;
  selectedAppNames: string[];
  onSuccess: () => void;
}

type StageMessage = {
  label: string;
  detail?: string;
  percent?: number;
};

function describe(event: ApplyEvent): StageMessage {
  switch (event.stage) {
    case "detecting":
      return { label: `Scanning ${prettyLauncher(event.launcher)}…` };
    case "writing-shortcuts":
      return { label: "Writing shortcuts.vdf…" };
    case "downloading-art":
      return {
        label: "Downloading cover art",
        detail: event.game,
        percent: Math.round((event.current / event.total) * 100),
      };
  }
}

function prettyLauncher(tag: string) {
  return tag === "epicstore" ? "Epic Games Store" : tag === "xbox" ? "Xbox" : tag;
}

export default function ApplyView({ options, selectedAppNames, onSuccess }: Props) {
  const [confirming, setConfirming] = useState(false);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<StageMessage | null>(null);
  const [result, setResult] = useState<ApplyResult | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    return () => {
      unlistenRef.current?.();
    };
  }, []);

  const runApply = async () => {
    setConfirming(false);
    setRunning(true);
    setResult(null);
    setProgress({ label: "Starting…" });

    // Subscribe to progress events for the duration of this run.
    onApplyProgress((evt) => setProgress(describe(evt))).then((unlisten) => {
      unlistenRef.current = unlisten;
    });

    try {
      const r = await applyChanges(options, selectedAppNames);
      setResult(r);
      if (!r.error) {
        onSuccess();
      }
    } catch (e) {
      setResult({ error: String(e) });
    } finally {
      setRunning(false);
      unlistenRef.current?.();
      unlistenRef.current = null;
    }
  };

  if (running) {
    return (
      <Card>
        <Space direction="vertical" style={{ width: "100%" }} size="large">
          <Typography.Title level={4} style={{ marginBottom: 0 }}>
            {progress?.label ?? "Working…"}
          </Typography.Title>
          {progress?.detail && (
            <Text type="secondary">{progress.detail}</Text>
          )}
          <Progress
            percent={progress?.percent ?? undefined}
            status="active"
            showInfo={progress?.percent != null}
          />
          <Text type="secondary" style={{ fontSize: 12 }}>
            Your old shortcuts file is being backed up automatically — you can
            close this window if needed, but it's faster if you let it finish.
          </Text>
        </Space>
      </Card>
    );
  }

  if (result?.error) {
    return (
      <Space direction="vertical" style={{ width: "100%" }} size="middle">
        <Alert
          type="error"
          showIcon
          message="Something went wrong"
          description={result.error}
        />
        <Button onClick={runApply}>Try again</Button>
      </Space>
    );
  }

  if (result) {
    const nothingToDo = (result.added ?? 0) === 0 && (result.removed ?? 0) === 0;
    return (
      <Result
        status="success"
        title={nothingToDo ? "Already up to date" : "Done!"}
        subTitle={
          nothingToDo
            ? `No changes needed — every selected game was already a shortcut on ${result.username ?? "your account"}.`
            : `Updated ${result.username ?? ""}'s library: added ${result.added ?? 0}, removed ${result.removed ?? 0}.`
        }
        extra={
          <Space direction="vertical" align="center">
            <Alert
              type="info"
              showIcon
              message="Restart Steam to see your new shortcuts."
              style={{ textAlign: "left" }}
            />
          </Space>
        }
      />
    );
  }

  // Idle: show the pre-apply summary + confirm button.
  return (
    <Card>
      <Space direction="vertical" size="large" style={{ width: "100%" }}>
        <Typography.Title level={4} style={{ marginBottom: 0 }}>
          Ready to add {selectedAppNames.length} game
          {selectedAppNames.length === 1 ? "" : "s"} to Steam
        </Typography.Title>

        <Space size="large" wrap>
          <Statistic
            title="Steam account"
            value={options.steamid || "—"}
            valueStyle={{ fontSize: 16 }}
          />
          <Statistic
            title="Games to add"
            value={selectedAppNames.length}
            prefix={<SaveOutlined />}
          />
          {options.download_art && (
            <Statistic
              title="Will fetch art"
              value="yes"
              prefix={<PictureOutlined />}
            />
          )}
        </Space>

        <Alert
          type="success"
          showIcon
          icon={<CheckCircleOutlined />}
          message="Safe to run"
          description={
            <Paragraph style={{ marginBottom: 0 }}>
              Your existing <Text code>shortcuts.vdf</Text> will be backed up
              automatically before any change. You can restore it if anything
              goes wrong.
            </Paragraph>
          }
        />

        <Button
          type="primary"
          size="large"
          block
          onClick={() => setConfirming(true)}
        >
          Add {selectedAppNames.length} game{selectedAppNames.length === 1 ? "" : "s"} to Steam
        </Button>
      </Space>

      <Modal
        open={confirming}
        title={
          <Space>
            <ExclamationCircleOutlined style={{ color: "#faad14" }} />
            <span>Add these to Steam?</span>
          </Space>
        }
        okText={`Yes, add ${selectedAppNames.length} game${selectedAppNames.length === 1 ? "" : "s"}`}
        cancelText="Wait, let me check"
        onOk={runApply}
        onCancel={() => setConfirming(false)}
        centered
      >
        <Paragraph>
          This will add <Text strong>{selectedAppNames.length}</Text> shortcut
          {selectedAppNames.length === 1 ? "" : "s"} to{" "}
          <Text strong>{options.steamid}</Text>'s Steam library.
        </Paragraph>
        <Paragraph type="secondary" style={{ marginBottom: 0 }}>
          A backup of your current Steam shortcuts file will be created first,
          so you can roll back if needed.
        </Paragraph>
      </Modal>
    </Card>
  );
}
