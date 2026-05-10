import { useEffect, useMemo, useRef, useState } from "react";
import {
  Alert,
  Button,
  Card,
  Empty,
  Image,
  Modal,
  Progress,
  Result,
  Space,
  Spin,
  Statistic,
  Typography,
} from "antd";
import {
  CheckCircleOutlined,
  ExclamationCircleOutlined,
  PictureOutlined,
  SaveOutlined,
} from "@ant-design/icons";
import {
  applyChanges,
  fetchArtPreviews,
  onApplyProgress,
  type ApplyEvent,
  type ArtPreview,
} from "../api";
import type { ApplyResult, Game, SyncOptions } from "../types";

const { Paragraph, Text } = Typography;

interface Props {
  options: SyncOptions;
  selectedGames: Game[];
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

/** Compact placeholder when SGDB has no match for a game. */
function PreviewPlaceholder({ name }: { name: string }) {
  return (
    <div
      style={{
        width: 120,
        height: 180,
        borderRadius: 8,
        background:
          "linear-gradient(135deg, rgba(91,108,255,0.18), rgba(91,108,255,0.04))",
        border: "1px dashed rgba(91,108,255,0.4)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 8,
        textAlign: "center",
      }}
    >
      <Text type="secondary" style={{ fontSize: 11 }}>
        No art
        <br />
        for &ldquo;{name.slice(0, 20)}
        {name.length > 20 ? "…" : ""}&rdquo;
      </Text>
    </div>
  );
}

export default function ApplyView({ options, selectedGames, onSuccess }: Props) {
  const [confirming, setConfirming] = useState(false);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<StageMessage | null>(null);
  const [result, setResult] = useState<ApplyResult | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  // Art preview state. We re-fetch whenever the user changes selection
  // or toggles art on/off, so they always see a live preview.
  const [previews, setPreviews] = useState<ArtPreview[] | null>(null);
  const [previewsLoading, setPreviewsLoading] = useState(false);

  const selectedAppNames = useMemo(
    () => selectedGames.map((g) => g.app_name),
    [selectedGames],
  );

  // Trigger preview fetch when selection or art config changes.
  // The current view is the only place we hit SGDB pre-write, so this
  // also doubles as a "live key works?" probe — if the key is wrong,
  // every preview will fail and the user notices before the real run.
  useEffect(() => {
    if (!options.download_art || !options.steamgriddb_api_key.trim()) {
      setPreviews(null);
      return;
    }
    if (selectedGames.length === 0) {
      setPreviews([]);
      return;
    }
    const names = selectedGames.map((g) => g.display_name);
    const key = options.steamgriddb_api_key;
    setPreviewsLoading(true);
    let cancelled = false;
    fetchArtPreviews(key, names)
      .then((rows) => {
        if (!cancelled) setPreviews(rows);
      })
      .catch(() => {
        if (!cancelled) setPreviews([]);
      })
      .finally(() => {
        if (!cancelled) setPreviewsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [options.download_art, options.steamgriddb_api_key, selectedGames]);

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
          {progress?.detail && <Text type="secondary">{progress.detail}</Text>}
          <Progress
            percent={progress?.percent ?? undefined}
            status="active"
            showInfo={progress?.percent != null}
          />
          <Text type="secondary" style={{ fontSize: 12 }}>
            Your old shortcuts file is being backed up automatically. You can
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

  const previewsMatched =
    previews?.filter((p) => p.box_art_url).length ?? 0;

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
              title="Cover art previewed"
              value={
                previewsLoading
                  ? "…"
                  : `${previewsMatched} of ${selectedGames.length}`
              }
              prefix={<PictureOutlined />}
            />
          )}
        </Space>

        {/* Cover art preview grid */}
        {options.download_art && options.steamgriddb_api_key.trim() && (
          <div>
            <Typography.Title level={5} style={{ marginTop: 0 }}>
              <PictureOutlined /> Cover art preview
            </Typography.Title>
            {previewsLoading && previews === null ? (
              <div style={{ textAlign: "center", padding: 24 }}>
                <Spin />
                <Paragraph type="secondary" style={{ marginTop: 8 }}>
                  Looking up art on SteamGridDB…
                </Paragraph>
              </div>
            ) : previews && previews.length > 0 ? (
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(auto-fill, 120px)",
                  gap: 16,
                  maxHeight: 460,
                  overflowY: "auto",
                  padding: "4px 0",
                }}
              >
                {previews.map((p) => (
                  <div
                    key={p.display_name}
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "center",
                      gap: 4,
                    }}
                  >
                    {p.box_art_url ? (
                      <Image
                        src={p.box_art_url}
                        width={120}
                        height={180}
                        style={{ objectFit: "cover", borderRadius: 8 }}
                        preview={{ mask: "View" }}
                        fallback="data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxMjAiIGhlaWdodD0iMTgwIi8+"
                      />
                    ) : (
                      <PreviewPlaceholder name={p.display_name} />
                    )}
                    <Text
                      ellipsis={{ tooltip: p.display_name }}
                      style={{ fontSize: 11, maxWidth: 120, textAlign: "center" }}
                    >
                      {p.display_name}
                    </Text>
                  </div>
                ))}
              </div>
            ) : (
              <Empty
                image={Empty.PRESENTED_IMAGE_SIMPLE}
                description="No previews yet"
              />
            )}
            {previews && previews.length > 0 && previewsMatched < previews.length && (
              <Alert
                type="warning"
                showIcon
                style={{ marginTop: 8 }}
                message={`${previews.length - previewsMatched} game${
                  previews.length - previewsMatched === 1 ? "" : "s"
                } have no SteamGridDB match — they'll get a default Steam placeholder.`}
              />
            )}
          </div>
        )}

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
          Add {selectedAppNames.length} game
          {selectedAppNames.length === 1 ? "" : "s"} to Steam
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
        okText={`Yes, add ${selectedAppNames.length} game${
          selectedAppNames.length === 1 ? "" : "s"
        }`}
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
