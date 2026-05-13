import React, { useEffect, useMemo, useRef, useState } from "react";
import {
  Alert,
  Button,
  Empty,
  Image,
  Input,
  Modal,
  Progress,
  Result,
  Space,
  Spin,
  Typography,
} from "antd";
import {
  CheckCircleOutlined,
  ExclamationCircleOutlined,
  PictureOutlined,
  SaveOutlined,
  SearchOutlined,
} from "@ant-design/icons";
import {
  applyChanges,
  fetchArtPreviews,
  onApplyProgress,
  restartSteam,
  type ApplyEvent,
  type ArtPreview,
} from "../api";
import type { ApplyResult, Game, SyncOptions } from "../types";

const { Paragraph, Text } = Typography;

interface Props {
  options: SyncOptions;
  selectedGames: Game[];
  exeOverrides: Record<string, string>;
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

function ArtThumb({
  url,
  label,
  width,
  height,
  fit = "cover",
}: {
  url: string | null | undefined;
  label: string;
  width: number;
  height: number;
  fit?: "cover" | "contain";
}) {
  if (url) {
    return (
      <Image
        src={url}
        width={width}
        height={height}
        style={{ objectFit: fit, borderRadius: 3, display: "block" }}
        preview={{ mask: label }}
      />
    );
  }
  return (
    <div className="ss-art-placeholder" style={{ width, height }}>
      {label}
    </div>
  );
}

function Stat({
  label,
  value,
  icon,
  variant,
}: {
  label: string;
  value: React.ReactNode;
  icon?: React.ReactNode;
  variant?: "default" | "action";
}) {
  return (
    <div className={`ss-stat${variant === "action" ? " action" : ""}`}>
      <div className="label">{label}</div>
      <div className="value">
        {icon}
        <span>{value}</span>
      </div>
    </div>
  );
}

export default function ApplyView({ options, selectedGames, exeOverrides, onSuccess }: Props) {
  const [confirming, setConfirming] = useState(false);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<StageMessage | null>(null);
  const [result, setResult] = useState<ApplyResult | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  const [previews, setPreviews] = useState<ArtPreview[] | null>(null);
  const [previewsLoading, setPreviewsLoading] = useState(false);

  const [editingArt, setEditingArt] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const [retryingArt, setRetryingArt] = useState<string | null>(null);

  const [nameOverrides, setNameOverrides] = useState<Record<string, string>>({});

  const selectedAppNames = useMemo(
    () => selectedGames.map((g) => g.app_name),
    [selectedGames],
  );

  const retryArtSearch = async (gameIndex: number, appName: string, searchTerm: string) => {
    if (!searchTerm.trim()) return;
    setEditingArt(null);
    setRetryingArt(appName);
    try {
      const [result] = await fetchArtPreviews(options.steamgriddb_api_key, [searchTerm.trim()]);
      setPreviews((prev) => {
        if (!prev) return prev;
        const next = [...prev];
        next[gameIndex] = {
          display_name: selectedGames[gameIndex].display_name,
          sgdb_name: result?.sgdb_name ?? null,
          box_art_url: result?.box_art_url ?? null,
          hero_url: result?.hero_url ?? null,
          logo_url: result?.logo_url ?? null,
          wide_url: result?.wide_url ?? null,
        };
        return next;
      });
      const overrideName = result?.sgdb_name ?? (result?.box_art_url ? searchTerm.trim() : null);
      if (overrideName) {
        setNameOverrides((prev) => ({ ...prev, [appName]: overrideName }));
      }
    } catch {
      // leave the card as no-art
    } finally {
      setRetryingArt(null);
    }
  };

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
        if (cancelled) return;
        setPreviews(rows);
        const auto: Record<string, string> = {};
        rows.forEach((row, i) => {
          if (row.sgdb_name) auto[selectedGames[i].app_name] = row.sgdb_name;
        });
        if (Object.keys(auto).length > 0) setNameOverrides((prev) => ({ ...prev, ...auto }));
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
      const r = await applyChanges(options, selectedAppNames, nameOverrides, exeOverrides);
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
      <section className="ss-panel">
        <header className="ss-panel-header">
          <span className="ss-panel-bar" />
          <h2 className="ss-panel-title">Deploying</h2>
        </header>
        <Space direction="vertical" style={{ width: "100%" }} size="large">
          <div>
            <div
              style={{
                fontFamily: '"Rajdhani", Inter, sans-serif',
                fontSize: 20,
                fontWeight: 600,
                letterSpacing: "0.04em",
                textTransform: "uppercase",
                color: "var(--ss-text-bright)",
              }}
            >
              {progress?.label ?? "Working…"}
            </div>
            {progress?.detail && (
              <Text type="secondary" style={{ fontSize: 13 }}>
                {progress.detail}
              </Text>
            )}
          </div>
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
      </section>
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
      <section className="ss-panel" style={{ padding: "32px 22px" }}>
        <Result
          status="success"
          title={nothingToDo ? "Already Synced" : "Mission Complete"}
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
              {options.steam_path && (
                <Button
                  type="primary"
                  onClick={() => restartSteam(options.steam_path)}
                >
                  Restart Steam now
                </Button>
              )}
            </Space>
          }
        />
      </section>
    );
  }

  const previewsMatched =
    previews?.filter((p) => p.box_art_url).length ?? 0;

  return (
    <Space direction="vertical" style={{ width: "100%" }} size="large">
      <section className="ss-panel">
        <header className="ss-panel-header">
          <span className="ss-panel-bar" />
          <h2 className="ss-panel-title">
            Ready to Deploy · {selectedAppNames.length} Game
            {selectedAppNames.length === 1 ? "" : "s"}
          </h2>
        </header>

        <div className="ss-stats">
          <Stat
            label="Steam account"
            value={options.steamid || "—"}
          />
          <Stat
            label="Games to add"
            value={selectedAppNames.length}
            icon={<SaveOutlined style={{ color: "var(--ss-accent)" }} />}
            variant="action"
          />
          {options.download_art && (
            <Stat
              label="Cover art matched"
              value={
                previewsLoading
                  ? "…"
                  : `${previewsMatched}/${selectedGames.length}`
              }
              icon={<PictureOutlined style={{ color: "var(--ss-accent)" }} />}
            />
          )}
        </div>
      </section>

      {options.download_art && options.steamgriddb_api_key.trim() && (
        <section className="ss-panel">
          <header className="ss-panel-header">
            <span className="ss-panel-bar" />
            <h2 className="ss-panel-title">Cover Art Preview</h2>
            <span style={{ flex: 1 }} />
            {previews && previews.length > 0 && (
              <Text
                type="secondary"
                style={{
                  fontSize: 11,
                  letterSpacing: "0.12em",
                  textTransform: "uppercase",
                }}
              >
                <Text strong style={{ color: "var(--ss-accent)", fontSize: 14 }}>
                  {previewsMatched}
                </Text>{" "}
                / {previews.length} matched
              </Text>
            )}
          </header>

          {previewsLoading && previews === null ? (
            <div style={{ textAlign: "center", padding: 32 }}>
              <Spin />
              <Paragraph type="secondary" style={{ marginTop: 12, fontSize: 12 }}>
                Looking up art on SteamGridDB…
              </Paragraph>
            </div>
          ) : previews && previews.length > 0 ? (
            <div className="ss-art-grid">
              {selectedGames.map((game, i) => {
                const p = previews[i];
                const isEditing = editingArt === game.app_name;
                const isRetrying = retryingArt === game.app_name;
                const hasArt = !!p?.box_art_url;
                return (
                  <div key={game.app_name} className="ss-art-card">
                    <div className="ss-art-row">
                      {isRetrying ? (
                        <div
                          className="ss-art-placeholder"
                          style={{ width: 88, height: 132 }}
                        >
                          <Spin size="small" />
                        </div>
                      ) : (
                        <ArtThumb url={p?.box_art_url} label="Cover" width={88} height={132} />
                      )}
                      <div style={{ display: "flex", flexDirection: "column", gap: 4, flex: 1 }}>
                        <ArtThumb url={p?.wide_url} label="Wide" width={136} height={64} />
                        <ArtThumb url={p?.hero_url} label="Background" width={136} height={44} />
                        <ArtThumb url={p?.logo_url} label="Logo" width={136} height={36} fit="contain" />
                      </div>
                    </div>

                    <div>
                      <Text
                        ellipsis={{ tooltip: nameOverrides[game.app_name] ?? game.display_name }}
                        style={{ fontSize: 12, display: "block", fontWeight: 500 }}
                      >
                        {nameOverrides[game.app_name] ?? game.display_name}
                      </Text>
                      {!hasArt && !isRetrying && (
                        isEditing ? (
                          <Space direction="vertical" size={4} style={{ width: "100%", marginTop: 4 }}>
                            <Input
                              size="small"
                              autoFocus
                              value={editValue}
                              onChange={(e) => setEditValue(e.target.value)}
                              onPressEnter={() => retryArtSearch(i, game.app_name, editValue)}
                              placeholder="Search SGDB…"
                            />
                            <Space size={4}>
                              <Button
                                size="small"
                                type="primary"
                                icon={<SearchOutlined />}
                                onClick={() => retryArtSearch(i, game.app_name, editValue)}
                                disabled={!editValue.trim()}
                              >
                                Search
                              </Button>
                              <Button size="small" onClick={() => setEditingArt(null)}>✕</Button>
                            </Space>
                          </Space>
                        ) : (
                          <Button
                            type="link"
                            size="small"
                            icon={<SearchOutlined />}
                            style={{ fontSize: 11, padding: 0, height: "auto", marginTop: 4 }}
                            onClick={() => {
                              setEditValue(game.display_name);
                              setEditingArt(game.app_name);
                            }}
                          >
                            Try search…
                          </Button>
                        )
                      )}
                    </div>
                  </div>
                );
              })}
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
              style={{ marginTop: 12 }}
              message={`${previews.length - previewsMatched} game${
                previews.length - previewsMatched === 1 ? "" : "s"
              } have no SteamGridDB match — use "Try search…" to find art with a different name, or continue without.`}
            />
          )}
        </section>
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
        className="ss-cta-action"
        onClick={() => setConfirming(true)}
      >
        ▶ Deploy {selectedAppNames.length} Game
        {selectedAppNames.length === 1 ? "" : "s"} to Steam
      </Button>

      <Modal
        open={confirming}
        title={
          <Space>
            <ExclamationCircleOutlined style={{ color: "var(--ss-warning)" }} />
            <span>Confirm deployment</span>
          </Space>
        }
        okText={`Yes, deploy ${selectedAppNames.length} game${
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
    </Space>
  );
}
