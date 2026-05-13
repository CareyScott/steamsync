import { useMemo, useState } from "react";
import {
  Badge,
  Button,
  Collapse,
  Empty,
  Input,
  Select,
  Space,
  Table,
  Tag,
  Tooltip,
  Typography,
  message,
} from "antd";
import {
  CheckCircleTwoTone,
  DeleteOutlined,
  FolderOpenOutlined,
  PlusCircleOutlined,
  WarningTwoTone,
  ReloadOutlined,
  SearchOutlined,
} from "@ant-design/icons";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { detectGames } from "../api";
import {
  SOURCE_LABELS,
  type Game,
  type GameStatus,
  type SteamAccount,
  type SyncOptions,
} from "../types";

const { Text } = Typography;

interface Props {
  options: SyncOptions;
  onOptionsChange: (o: SyncOptions) => void;
  games: Game[];
  setGames: (g: Game[]) => void;
  accounts: SteamAccount[];
  setAccounts: (a: SteamAccount[]) => void;
  selected: Set<string>;
  setSelected: (s: Set<string>) => void;
  exeOverrides: Record<string, string>;
  setExeOverrides: (e: Record<string, string>) => void;
  onProceed: () => void;
}

const SOURCES: { value: string; label: string }[] = [
  { value: "epicstore", label: "Epic Games" },
  { value: "xbox", label: "Xbox" },
  { value: "local", label: "Local Folders" },
];

function statusFor(game: Game, existing: Set<string>): GameStatus {
  return existing.has(game.app_name) ? "synced" : "new";
}

function StatusIcon({ status }: { status: GameStatus }) {
  switch (status) {
    case "synced":
      return (
        <Tooltip title="Already in Steam">
          <CheckCircleTwoTone twoToneColor="#a4d007" />
        </Tooltip>
      );
    case "broken":
      return (
        <Tooltip title="Executable missing or unreadable">
          <WarningTwoTone twoToneColor="#e0a225" />
        </Tooltip>
      );
    case "new":
      return (
        <Tooltip title="Will be added to Steam">
          <PlusCircleOutlined style={{ color: "#66c0f4" }} />
        </Tooltip>
      );
    default:
      return <span>·</span>;
  }
}

function exeLabel(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] ?? path;
}

export default function DetectView(props: Props) {
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState("");
  const [existingAppNames, setExistingAppNames] = useState<Set<string>>(new Set());

  const set = (patch: Partial<SyncOptions>) =>
    props.onOptionsChange({ ...props.options, ...patch });

  const toggleSource = (value: string) => {
    const has = props.options.sources.includes(value);
    set({
      sources: has
        ? props.options.sources.filter((s) => s !== value)
        : [...props.options.sources, value],
    });
  };

  const handleAddFolders = async () => {
    try {
      const result = await openFolderDialog({ directory: true, multiple: true });
      if (!result) return;
      const chosen = Array.isArray(result) ? result : [result];
      const merged = [...new Set([...props.options.local_folders, ...chosen])];
      set({ local_folders: merged });
    } catch (e) {
      message.error(`Could not open folder picker: ${String(e)}`);
    }
  };

  const removeFolder = (folder: string) =>
    set({ local_folders: props.options.local_folders.filter((f) => f !== folder) });

  const handleDetect = async () => {
    setLoading(true);
    try {
      const result = await detectGames(props.options);
      if (result.error) {
        message.error(`Detect failed: ${result.error}`);
        return;
      }
      props.setGames(result.games);
      props.setAccounts(result.accounts);
      const existing = new Set(result.existing_app_names);
      setExistingAppNames(existing);
      props.setSelected(
        new Set(result.games.filter((g) => !existing.has(g.app_name)).map((g) => g.app_name)),
      );
      if (result.accounts.length === 1 && !props.options.steamid) {
        props.onOptionsChange({
          ...props.options,
          steamid: result.accounts[0].steamid,
        });
      }
      message.success(
        `Found ${result.games.length} games across ${
          new Set(result.games.map((g) => g.storetag)).size
        } provider(s), ${result.accounts.length} steam account(s).`,
      );
    } catch (e) {
      message.error(`Detect failed: ${String(e)}`);
    } finally {
      setLoading(false);
    }
  };

  const grouped = useMemo(() => {
    const q = search.trim().toLowerCase();
    const by: Record<string, Game[]> = {};
    for (const g of props.games) {
      if (q && !g.display_name.toLowerCase().includes(q)) continue;
      (by[g.storetag] ||= []).push(g);
    }
    return by;
  }, [props.games, search]);

  const toggleOne = (appName: string, on: boolean) => {
    const next = new Set(props.selected);
    if (on) next.add(appName);
    else next.delete(appName);
    props.setSelected(next);
  };

  const setProviderSelection = (tag: string, on: boolean) => {
    const games = grouped[tag] || [];
    const next = new Set(props.selected);
    for (const g of games) {
      if (on) next.add(g.app_name);
      else next.delete(g.app_name);
    }
    props.setSelected(next);
  };

  const selectAllNew = () => {
    const next = new Set(props.selected);
    for (const g of props.games) {
      if (statusFor(g, existingAppNames) === "new") next.add(g.app_name);
    }
    props.setSelected(next);
  };

  const clearAll = () => props.setSelected(new Set());

  const totalSelected = props.selected.size;
  const totalGames = props.games.length;
  const selectedUpdates = props.games.filter(
    (g) => props.selected.has(g.app_name) && existingAppNames.has(g.app_name),
  ).length;
  const selectedNew = totalSelected - selectedUpdates;

  const { options } = props;

  return (
    <Space direction="vertical" style={{ width: "100%" }} size="large">
      <section className="ss-panel">
        <header className="ss-panel-header">
          <span className="ss-panel-bar" />
          <h2 className="ss-panel-title">Sources</h2>
        </header>

        <div className="ss-source-row">
          {SOURCES.map((src) => {
            const checked = options.sources.includes(src.value);
            return (
              <button
                key={src.value}
                type="button"
                className={`ss-source-pill ${checked ? "checked" : ""}`}
                onClick={() => toggleSource(src.value)}
              >
                <span className="dot" />
                {src.label}
              </button>
            );
          })}
        </div>

        {options.sources.includes("local") && (
          <div style={{ marginTop: 16 }}>
            <h3 className="ss-section-heading">Local Folders</h3>
            <Space direction="vertical" size={6} style={{ width: "100%" }}>
              {options.local_folders.length === 0 && (
                <Text type="secondary" style={{ fontSize: 13 }}>
                  No folders added yet — each direct subfolder becomes one game.
                </Text>
              )}
              {options.local_folders.map((folder) => (
                <div
                  key={folder}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    padding: "6px 10px",
                    background: "var(--ss-bg-elevated)",
                    border: "1px solid var(--ss-border)",
                    borderRadius: 4,
                  }}
                >
                  <FolderOpenOutlined style={{ color: "var(--ss-accent)" }} />
                  <Text
                    style={{
                      flex: 1,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                      fontSize: 13,
                    }}
                    title={folder}
                  >
                    {folder}
                  </Text>
                  <Button
                    type="text"
                    danger
                    size="small"
                    icon={<DeleteOutlined />}
                    onClick={() => removeFolder(folder)}
                  />
                </div>
              ))}
              <Button size="small" icon={<FolderOpenOutlined />} onClick={handleAddFolders}>
                Add folder…
              </Button>
            </Space>
          </div>
        )}

        <div style={{ marginTop: 20, display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
          <Button
            type="primary"
            size="large"
            loading={loading}
            icon={<ReloadOutlined />}
            onClick={handleDetect}
          >
            {totalGames > 0 ? "Scan Again" : "Scan for Games"}
          </Button>
          {options.steam_path && (
            <Text type="secondary" style={{ fontSize: 12 }}>
              Steam folder:{" "}
              <Text code style={{ fontSize: 11 }}>
                {options.steam_path}
              </Text>
            </Text>
          )}
        </div>
      </section>

      {totalGames === 0 && !loading && (
        <div className="ss-panel" style={{ textAlign: "center", padding: "40px 24px" }}>
          <Empty
            description={
              <Text type="secondary" style={{ letterSpacing: "0.04em" }}>
                Ready when you are. Hit{" "}
                <Text strong style={{ color: "var(--ss-accent)" }}>
                  Scan for Games
                </Text>{" "}
                to begin.
              </Text>
            }
            image={Empty.PRESENTED_IMAGE_SIMPLE}
          />
        </div>
      )}

      {totalGames > 0 && (
        <section className="ss-panel">
          <header className="ss-panel-header">
            <span className="ss-panel-bar" />
            <h2 className="ss-panel-title">Library</h2>
            <span style={{ flex: 1 }} />
            <Text
              type="secondary"
              style={{
                fontSize: 11,
                letterSpacing: "0.12em",
                textTransform: "uppercase",
              }}
            >
              <Text strong style={{ color: "var(--ss-accent)", fontSize: 14 }}>
                {totalSelected}
              </Text>{" "}
              / {totalGames} selected
            </Text>
          </header>

          <div
            style={{
              display: "flex",
              gap: 8,
              marginBottom: 16,
              flexWrap: "wrap",
              alignItems: "center",
            }}
          >
            <Input
              prefix={<SearchOutlined style={{ color: "var(--ss-text-muted)" }} />}
              placeholder="Filter by name…"
              allowClear
              style={{ width: 320 }}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            <Button onClick={selectAllNew}>Select all new</Button>
            <Button onClick={clearAll}>Clear</Button>
          </div>

          <Collapse
            defaultActiveKey={Object.keys(grouped)}
            items={Object.entries(grouped).map(([tag, games]) => {
              const selectedInGroup = games.filter((g) =>
                props.selected.has(g.app_name),
              ).length;
              return {
                key: tag,
                label: (
                  <Space>
                    <span
                      style={{
                        fontFamily: '"Rajdhani", Inter, sans-serif',
                        fontWeight: 600,
                        fontSize: 14,
                        letterSpacing: "0.06em",
                        textTransform: "uppercase",
                        color: "var(--ss-text-bright)",
                      }}
                    >
                      {SOURCE_LABELS[tag] ?? tag}
                    </span>
                    <Badge
                      count={`${selectedInGroup} / ${games.length}`}
                      style={{
                        backgroundColor: "var(--ss-bg-base)",
                        color: "var(--ss-accent)",
                        boxShadow: "inset 0 0 0 1px var(--ss-accent)",
                        fontFamily: '"Rajdhani", Inter, sans-serif',
                      }}
                    />
                  </Space>
                ),
                extra: (
                  <Space onClick={(e) => e.stopPropagation()}>
                    <Button
                      size="small"
                      onClick={() => setProviderSelection(tag, true)}
                    >
                      All
                    </Button>
                    <Button
                      size="small"
                      onClick={() => setProviderSelection(tag, false)}
                    >
                      None
                    </Button>
                  </Space>
                ),
                children: (
                  <div style={{ overflow: "hidden" }}>
                    <Table<Game>
                      rowKey="app_name"
                      size="small"
                      pagination={false}
                      dataSource={games}
                      scroll={{ x: "max-content" }}
                      columns={[
                        {
                          title: "",
                          width: 40,
                          align: "center",
                          render: (_v, g) => <StatusIcon status={statusFor(g, existingAppNames)} />,
                        },
                        {
                          title: "Name",
                          dataIndex: "display_name",
                          ellipsis: true,
                        },
                        {
                          title: "App ID",
                          dataIndex: "app_name",
                          ellipsis: true,
                          width: 280,
                          render: (v) => (
                            <Text type="secondary" code style={{ fontSize: 12 }}>
                              {v}
                            </Text>
                          ),
                        },
                        {
                          title: "Executable",
                          width: 240,
                          render: (_v, g) => {
                            if (g.storetag !== "local" || g.exe_candidates.length <= 1) return null;
                            const current = props.exeOverrides[g.app_name] ?? g.executable_path;
                            const isLauncher = (p: string) =>
                              exeLabel(p).toLowerCase().includes("launcher");
                            return (
                              <Tooltip title="Pick which executable to launch. If the game won't open, try switching to the launcher.">
                                <Select
                                  size="small"
                                  style={{ width: "100%" }}
                                  value={current}
                                  onChange={(v) =>
                                    props.setExeOverrides({ ...props.exeOverrides, [g.app_name]: v })
                                  }
                                  options={g.exe_candidates.map((p, idx) => ({
                                    value: p,
                                    label: (
                                      <span>
                                        {exeLabel(p)}
                                        {isLauncher(p) && (
                                          <Text type="secondary" style={{ fontSize: 11, marginLeft: 4 }}>
                                            (launcher)
                                          </Text>
                                        )}
                                        {idx === 0 && !isLauncher(p) && (
                                          <Text type="secondary" style={{ fontSize: 11, marginLeft: 4 }}>
                                            (largest)
                                          </Text>
                                        )}
                                      </span>
                                    ),
                                  }))}
                                />
                              </Tooltip>
                            );
                          },
                        },
                        {
                          title: "",
                          width: 110,
                          align: "right",
                          render: (_v, g) => {
                            const synced = existingAppNames.has(g.app_name);
                            const selected = props.selected.has(g.app_name);
                            const color = selected
                              ? synced
                                ? "orange"
                                : "blue"
                              : "default";
                            return (
                              <Tag
                                className="ss-pick-tag"
                                color={color}
                                onClick={() => toggleOne(g.app_name, !selected)}
                              >
                                {selected
                                  ? synced
                                    ? "✓ update"
                                    : "✓ queued"
                                  : synced
                                  ? "update"
                                  : "select"}
                              </Tag>
                            );
                          },
                        },
                      ]}
                    />
                  </div>
                ),
              };
            })}
          />

          <Button
            type="primary"
            size="large"
            block
            disabled={totalSelected === 0}
            onClick={props.onProceed}
            style={{ marginTop: 20 }}
          >
            {selectedNew > 0 && selectedUpdates > 0
              ? `Continue — ${selectedNew} new, ${selectedUpdates} update${selectedUpdates !== 1 ? "s" : ""} →`
              : selectedUpdates > 0
              ? `Continue — ${selectedUpdates} update${selectedUpdates !== 1 ? "s" : ""} →`
              : `Continue with ${selectedNew} game${selectedNew !== 1 ? "s" : ""} →`}
          </Button>
        </section>
      )}
    </Space>
  );
}
