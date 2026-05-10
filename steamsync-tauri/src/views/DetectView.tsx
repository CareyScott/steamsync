import { useMemo, useState } from "react";
import {
  Badge,
  Button,
  Collapse,
  Empty,
  Input,
  Space,
  Table,
  Tag,
  Tooltip,
  Typography,
  message,
} from "antd";
import {
  CheckCircleTwoTone,
  PlusCircleOutlined,
  WarningTwoTone,
  ReloadOutlined,
} from "@ant-design/icons";
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
  onProceed: () => void;
}

// Phase 1.5: every game shows "new" because we don't yet read shortcuts.vdf
// to compare. Phase 2 fills in real status.
function statusFor(_game: Game): GameStatus {
  return "new";
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

export default function DetectView(props: Props) {
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState("");

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
      // Default selection: every "new" game (Phase 1.5 = all of them).
      props.setSelected(
        new Set(
          result.games.filter((g) => statusFor(g) === "new").map((g) => g.app_name),
        ),
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
      if (statusFor(g) === "new") next.add(g.app_name);
    }
    props.setSelected(next);
  };

  const clearAll = () => props.setSelected(new Set());

  const totalSelected = props.selected.size;
  const totalGames = props.games.length;

  return (
    <Space direction="vertical" style={{ width: "100%" }} size="large">
      <Space wrap>
        <Text>Steam path:</Text>
        <Input
          style={{ width: 360 }}
          value={props.options.steam_path}
          onChange={(e) =>
            props.onOptionsChange({
              ...props.options,
              steam_path: e.target.value,
            })
          }
        />
        <Button
          type="primary"
          loading={loading}
          icon={<ReloadOutlined />}
          onClick={handleDetect}
        >
          {totalGames > 0 ? "Re-scan" : "Detect games"}
        </Button>
      </Space>

      {totalGames === 0 && !loading && (
        <Empty
          description="No games yet — click Detect games to scan your launchers."
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
                  <Table<Game>
                    rowKey="app_name"
                    size="small"
                    pagination={false}
                    dataSource={games}
                    columns={[
                      {
                        title: "",
                        width: 40,
                        align: "center",
                        render: (_v, g) => <StatusIcon status={statusFor(g)} />,
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
                        title: "",
                        width: 60,
                        align: "right",
                        render: (_v, g) => (
                          <Tag
                            color={props.selected.has(g.app_name) ? "blue" : "default"}
                            onClick={() =>
                              toggleOne(g.app_name, !props.selected.has(g.app_name))
                            }
                            style={{ cursor: "pointer", margin: 0 }}
                          >
                            {props.selected.has(g.app_name) ? "✓ selected" : "select"}
                          </Tag>
                        ),
                      },
                    ]}
                  />
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
            Continue with {totalSelected} game(s) →
          </Button>
        </>
      )}
    </Space>
  );
}
