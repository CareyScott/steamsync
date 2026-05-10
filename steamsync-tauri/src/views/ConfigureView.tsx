import {
  Button,
  Card,
  Checkbox,
  Form,
  Input,
  Select,
  Space,
  Switch,
  Typography,
} from "antd";
import type { SteamAccount, SyncOptions } from "../types";

const { Text } = Typography;

interface Props {
  options: SyncOptions;
  onOptionsChange: (o: SyncOptions) => void;
  accounts: SteamAccount[];
  selectedCount: number;
  totalGames: number;
  onProceed: () => void;
}

const SOURCE_OPTIONS = [
  { value: "epicstore", label: "Epic Games Store" },
  { value: "xbox", label: "Xbox" },
];

export default function ConfigureView(props: Props) {
  const { options, onOptionsChange, accounts, selectedCount, totalGames, onProceed } = props;
  const set = (patch: Partial<SyncOptions>) =>
    onOptionsChange({ ...options, ...patch });

  return (
    <Card>
      <Form layout="vertical">
        <Form.Item label="Steam account">
          {accounts.length > 0 ? (
            <Select
              value={options.steamid || undefined}
              placeholder="Select an account"
              onChange={(v) => set({ steamid: v })}
              options={accounts.map((a) => ({
                value: a.steamid,
                label: `${a.username} (${a.steamid})`,
              }))}
            />
          ) : (
            <Input
              value={options.steamid}
              onChange={(e) => set({ steamid: e.target.value })}
              placeholder="SteamID"
            />
          )}
        </Form.Item>

        <Form.Item label="Sources to scan">
          <Checkbox.Group
            options={SOURCE_OPTIONS}
            value={options.sources}
            onChange={(v) => set({ sources: v as string[] })}
          />
        </Form.Item>

        <Form.Item label="Behavior">
          <Space direction="vertical">
            <Space>
              <Switch
                checked={options.use_uri}
                onChange={(v) => set({ use_uri: v })}
              />
              <Text>Use launcher URI instead of executable path</Text>
            </Space>
            <Space>
              <Switch
                checked={options.replace_existing}
                onChange={(v) => set({ replace_existing: v })}
              />
              <Text>Replace existing shortcuts</Text>
            </Space>
            <Space>
              <Switch
                checked={options.remove_missing}
                onChange={(v) => set({ remove_missing: v })}
              />
              <Text>Remove shortcuts to missing games</Text>
            </Space>
            <Space>
              <Switch
                checked={options.download_art}
                onChange={(v) => set({ download_art: v })}
              />
              <Text>Download Steam grid art</Text>
            </Space>
          </Space>
        </Form.Item>

        {options.download_art && (
          <Form.Item
            label="Steam API key"
            help="Required when downloading art."
          >
            <Input.Password
              value={options.steam_api_key}
              onChange={(e) => set({ steam_api_key: e.target.value })}
            />
          </Form.Item>
        )}

        <Form.Item label="Epic Games Store manifests">
          <Input
            value={options.egs_manifests}
            onChange={(e) => set({ egs_manifests: e.target.value })}
          />
        </Form.Item>

        <Button
          type="primary"
          disabled={!options.steamid || selectedCount === 0}
          onClick={onProceed}
        >
          Apply {selectedCount} of {totalGames} games →
        </Button>
      </Form>
    </Card>
  );
}
