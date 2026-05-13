import { useMemo, useState } from "react";
import {
  Badge,
  Button,
  Checkbox,
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

function statusFor(game: Game, existing: Set<string>): GameStatus {
  return existing.has(game.app_name) ? "synced" : "new";
}

function StatusIcon({ status }: { status: GameStatus }) {
  switch (status) {
    case "synced":
      return (
        <Tooltip title="Already in Steam">
          <CheckCircleTwoTone twoToneColor="#52c41a" />
        </Tooltip>
      );
    case "broken":
      return (
        <Tooltip title="Executable missing or unreadable">
          <WarningTwoTone twoToneColor="#faad14" />
        </Tooltip>
      );
    case "new":
      return (
        <Tooltip title="Will be added to Steam">
          <PlusCircleOutlined style={{ color: "#1677ff" }} />
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
      // Default selection: every game not already in Steam.
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

  // Group games by storetag, filtered by search.
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
      <Space direction="vertical" size="small" style={{ width: "100%" }}>
        <Checkbox.Group
          value={options.sources}
          onChange={(v) => set({ sources: v as string[] })}
          options={[
            { value: "epicstore", label: "Epic Games Store" },
            { value: "xbox", label: "Xbox" },
            { value: "local", label: "Local Folders" },
          ]}
        />
        {options.sources.includes("local") && (
          <Space direction="vertical" size={4} style={{ paddingLeft: 2 }}>
            {options.local_folders.length === 0 && (
              <Text type="secondary" style={{ fontSize: 13 }}>
                No folders added yet — each direct subfolder becomes one game.
              </Text>
            )}
            {options.local_folders.map((folder) => (
              <Space key={folder} size="small" align="center">
                <FolderOpenOutlined style={{ opacity: 0.55 }} />
                <Text
                  style={{
                    maxWidth: 540,
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
              </Space>
            ))}
            <Button size="small" icon={<FolderOpenOutlined />} onClick={handleAddFolders}>
              Add folder…
            </Button>
          </Space>
        )}
      </Space>

      <Space wrap align="center">
        <Button
          type="primary"
          size="large"
          loading={loading}
          icon={<ReloadOutlined />}
          onClick={handleDetect}
        >
          {totalGames > 0 ? "Scan again" : "Find my games"}
        </Button>
        {options.steam_path && (
          <Text type="secondary">
            Steam folder: <Text code>{options.steam_path}</Text>
          </Text>
        )}
      </Space>

      {totalGames === 0 && !loading && (
        <Empty
          description="Click 'Find my games' to scan your selected sources."
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      )}

      {totalGames > 0 && (
        <>
          <Space wrap>
            <Input.Search
              placeholder="Filter by name…"
              allowClear
              style={{ width: 320 }}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            <Button onClick={selectAllNew}>Select all new</Button>
            <Button onClick={clearAll}>Clear</Button>
            <Text type="secondary">
              {totalSelected} of {totalGames} selected
            </Text>
          </Space>

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
                    <strong>{SOURCE_LABELS[tag] ?? tag}</strong>
                    <Badge
                      count={`${selectedInGroup} / ${games.length}`}
                      style={{ backgroundColor: "#1677ff" }}
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
                        width: 90,
                        align: "right",
                        render: (_v, g) => {
                          const synced = existingAppNames.has(g.app_name);
                          const selected = props.selected.has(g.app_name);
                          return (
                            <Tag
                              color={selected ? (synced ? "orange" : "blue") : "default"}
                              onClick={() => toggleOne(g.app_name, !selected)}
                              style={{ cursor: "pointer", margin: 0 }}
                            >
                              {selected
                                ? (synced ? "✓ update" : "✓ selected")
                                : (synced ? "Update" : "select")}
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
            disabled={totalSelected === 0}
            onClick={props.onProceed}
          >
            {selectedNew > 0 && selectedUpdates > 0
              ? `Continue — ${selectedNew} new, ${selectedUpdates} update${selectedUpdates !== 1 ? "s" : ""} →`
              : selectedUpdates > 0
              ? `Continue — ${selectedUpdates} update${selectedUpdates !== 1 ? "s" : ""} →`
              : `Continue with ${selectedNew} game${selectedNew !== 1 ? "s" : ""} →`}
          </Button>
        </>
      )}
    </Space>
  );
}
