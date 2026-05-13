import { useEffect, useState } from "react";
import {
  Alert,
  Button,
  Card,
  Collapse,
  Form,
  Input,
  Modal,
  Select,
  Space,
  Switch,
  Tag,
  Typography,
} from "antd";
import { ExportOutlined, InfoCircleOutlined, PictureOutlined } from "@ant-design/icons";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { SteamAccount, SyncOptions } from "../types";

const { Text, Paragraph } = Typography;

interface Props {
  options: SyncOptions;
  onOptionsChange: (o: SyncOptions) => void;
  accounts: SteamAccount[];
  selectedCount: number;
  totalGames: number;
  onProceed: () => void;
}

export default function ConfigureView(props: Props) {
  const { options, onOptionsChange, accounts, selectedCount, totalGames, onProceed } = props;
  const set = (patch: Partial<SyncOptions>) => onOptionsChange({ ...options, ...patch });

  // Onboarding modal triggers the first time the user toggles art on
  // without a key configured. We dismiss as soon as a key is set or the
  // user opts out, so it doesn't get in the way on subsequent runs.
  const [showSgdbModal, setShowSgdbModal] = useState(false);
  useEffect(() => {
    if (options.download_art && !options.steamgriddb_api_key.trim()) {
      setShowSgdbModal(true);
    }
  }, [options.download_art, options.steamgriddb_api_key]);

  return (
    <Card>
      <Form layout="vertical">
        <Form.Item
          label="Which Steam account?"
          help="Where new shortcuts will be added."
        >
          {accounts.length > 0 ? (
            <Select
              value={options.steamid || undefined}
              placeholder="Pick an account"
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

        <Form.Item>
          <Space direction="vertical" size="middle">
            <Space>
              <Switch
                checked={options.download_art}
                onChange={(v) => set({ download_art: v })}
              />
              <PictureOutlined />
              <Text>Download cover art (looks much nicer in Steam)</Text>
              {options.download_art && options.steamgriddb_api_key.trim() && (
                <Tag color="green">key set</Tag>
              )}
            </Space>
            <Space>
              <Switch
                checked={options.remove_missing}
                onChange={(v) => set({ remove_missing: v })}
              />
              <Text>Remove shortcuts whose games are gone</Text>
            </Space>
            <Space>
              <Switch
                checked={options.use_uri}
                onChange={(v) => set({ use_uri: v })}
              />
              <Text>
                Launch via the launcher's URI{" "}
                <Text type="secondary" style={{ fontSize: 12 }}>
                  (needed for some online games like GTAV)
                </Text>
              </Text>
            </Space>
          </Space>
        </Form.Item>

        <Collapse
          ghost
          items={[
            {
              key: "advanced",
              label: (
                <Text type="secondary">
                  <InfoCircleOutlined /> Advanced settings
                </Text>
              ),
              children: (
                <Space direction="vertical" style={{ width: "100%" }} size="small">
                  <Form.Item
                    label="Steam install path"
                    help="Auto-detected. Change only if you know what you're doing."
                    style={{ marginBottom: 12 }}
                  >
                    <Input
                      value={options.steam_path}
                      onChange={(e) => set({ steam_path: e.target.value })}
                      placeholder="e.g. C:\\Program Files (x86)\\Steam"
                    />
                  </Form.Item>
                  <Form.Item
                    label="Epic Games Store manifests folder"
                    help="Where Epic stores its .item files."
                    style={{ marginBottom: 12 }}
                  >
                    <Input
                      value={options.egs_manifests}
                      onChange={(e) => set({ egs_manifests: e.target.value })}
                    />
                  </Form.Item>
                  {options.download_art && (
                    <Form.Item
                      label="SteamGridDB API key"
                      help={
                        <span>
                          See the <a onClick={() => setShowSgdbModal(true)}>set-up guide</a> if
                          you don't have one yet.
                        </span>
                      }
                    >
                      <Input.Password
                        value={options.steamgriddb_api_key}
                        onChange={(e) => set({ steamgriddb_api_key: e.target.value })}
                        placeholder="paste your key here"
                      />
                    </Form.Item>
                  )}
                </Space>
              ),
            },
          ]}
        />

        <Button
          type="primary"
          size="large"
          block
          disabled={!options.steamid || selectedCount === 0}
          onClick={onProceed}
          style={{ marginTop: 16 }}
        >
          Continue with {selectedCount} of {totalGames} games →
        </Button>
      </Form>

      <Modal
        open={showSgdbModal}
        title={
          <Space>
            <PictureOutlined style={{ color: "#5b6cff" }} />
            <span>Set up cover art (one-time)</span>
          </Space>
        }
        onCancel={() => setShowSgdbModal(false)}
        footer={null}
        width={620}
        centered
      >
        <Paragraph>
          Cover art comes from <Text strong>SteamGridDB</Text> — a free
          community-curated database. It takes about a minute to set up and
          you only have to do it once.
        </Paragraph>

        <ol style={{ paddingLeft: 20, lineHeight: 1.9 }}>
          <li>
            <Button
              type="link"
              icon={<ExportOutlined />}
              style={{ padding: 0 }}
              onClick={() => openUrl("https://www.steamgriddb.com/register")}
            >
              Create a free SteamGridDB account
            </Button>{" "}
            (or sign in if you already have one).
          </li>
          <li>
            Once logged in, click your avatar in the top-right and pick{" "}
            <Text code>Preferences</Text>.
          </li>
          <li>
            Open the <Text code>API</Text> tab in the sidebar, then{" "}
            <Text code>Generate API Key</Text>.
          </li>
          <li>Copy the key and paste it below.</li>
        </ol>

        <Form.Item
          label="API key"
          style={{ marginTop: 16 }}
        >
          <Input.Password
            value={options.steamgriddb_api_key}
            onChange={(e) => set({ steamgriddb_api_key: e.target.value })}
            placeholder="paste the API key here"
            autoFocus
          />
        </Form.Item>

        <Alert
          type="info"
          showIcon
          message="Your key stays on this device."
          description="It's saved locally and only used to fetch art from SteamGridDB. Nothing is sent anywhere else."
          style={{ marginBottom: 16 }}
        />

        <Space style={{ width: "100%", justifyContent: "flex-end" }}>
          <Button
            onClick={() => {
              set({ download_art: false });
              setShowSgdbModal(false);
            }}
          >
            Skip cover art
          </Button>
          <Button
            type="primary"
            disabled={!options.steamgriddb_api_key.trim()}
            onClick={() => setShowSgdbModal(false)}
          >
            Done
          </Button>
        </Space>
      </Modal>
    </Card>
  );
}
