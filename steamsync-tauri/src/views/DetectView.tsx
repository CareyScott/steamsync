import { useState } from "react";
import { Button, Input, Space, Table, Typography, message } from "antd";
import { detectGames } from "../api";
import type { Game, SteamAccount, SyncOptions } from "../types";

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

export default function DetectView(props: Props) {
  const [loading, setLoading] = useState(false);

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
      props.setSelected(new Set(result.games.map((g) => g.app_name)));
      if (result.accounts.length === 1 && !props.options.steamid) {
        props.onOptionsChange({
          ...props.options,
          steamid: result.accounts[0].steamid,
        });
      }
      message.success(
        `Found ${result.games.length} games and ${result.accounts.length} steam account(s).`,
      );
    } catch (e) {
      message.error(`Detect failed: ${String(e)}`);
    } finally {
      setLoading(false);
    }
  };

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
        <Button type="primary" loading={loading} onClick={handleDetect}>
          Detect games
        </Button>
      </Space>
      {props.games.length > 0 && (
        <>
          <Table<Game>
            rowKey="app_name"
            size="small"
            pagination={{ pageSize: 20 }}
            dataSource={props.games}
            rowSelection={{
              selectedRowKeys: Array.from(props.selected),
              onChange: (keys) =>
                props.setSelected(new Set(keys.map(String))),
            }}
            columns={[
              { title: "Name", dataIndex: "display_name", ellipsis: true },
              { title: "Source", dataIndex: "storetag", width: 120 },
              { title: "App ID", dataIndex: "app_name", ellipsis: true },
            ]}
          />
          <Button
            type="primary"
            disabled={props.selected.size === 0}
            onClick={props.onProceed}
          >
            Continue with {props.selected.size} game(s) →
          </Button>
        </>
      )}
    </Space>
  );
}
