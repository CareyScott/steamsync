import { useEffect, useState } from "react";
import {
  Alert,
  Button,
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

function ToggleRow({
  checked,
  onChange,
  title,
  description,
  badge,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  title: React.ReactNode;
  description?: React.ReactNode;
  badge?: React.ReactNode;
}) {
  return (
    <label
      style={{
        display: "flex",
        alignItems: "flex-start",
        gap: 14,
        padding: "12px 14px",
        background: "var(--ss-bg-elevated)",
        border: `1px solid ${checked ? "var(--ss-action)" : "var(--ss-border)"}`,
        borderLeftWidth: 3,
        borderRadius: 4,
        cursor: "pointer",
        transition: "all 0.15s ease",
      }}
    >
      <Switch checked={checked} onChange={onChange} style={{ marginTop: 2 }} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <Text strong style={{ fontSize: 13 }}>
            {title}
          </Text>
          {badge}
        </div>
        {description && (
          <div style={{ fontSize: 12, color: "var(--ss-text-muted)", marginTop: 4 }}>
            {description}
          </div>
        )}
      </div>
    </label>
  );
}

export default function ConfigureView(props: Props) {
  const { options, onOptionsChange, accounts, selectedCount, totalGames, onProceed } = props;
  const set = (patch: Partial<SyncOptions>) => onOptionsChange({ ...options, ...patch });

  const [showSgdbModal, setShowSgdbModal] = useState(false);
  useEffect(() => {
    if (options.download_art && !options.steamgriddb_api_key.trim()) {
      setShowSgdbModal(true);
    }
  }, [options.download_art, options.steamgriddb_api_key]);

  return (
    <Space direction="vertical" style={{ width: "100%" }} size="large">
      <section className="ss-panel">
        <header className="ss-panel-header">
          <span className="ss-panel-bar" />
          <h2 className="ss-panel-title">Account</h2>
        </header>
        <Form layout="vertical">
          <Form.Item
            label="Target Steam account"
            help="Where new shortcuts will be added."
            style={{ marginBottom: 0 }}
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
        </Form>
      </section>

      <section className="ss-panel">
        <header className="ss-panel-header">
          <span className="ss-panel-bar" />
          <h2 className="ss-panel-title">Options</h2>
        </header>

        <Space direction="vertical" size={10} style={{ width: "100%" }}>
          <ToggleRow
            checked={options.download_art}
            onChange={(v) => set({ download_art: v })}
            title={
              <span>
                <PictureOutlined style={{ marginRight: 6, color: "var(--ss-accent)" }} />
                Download cover art
              </span>
            }
            description="Pulls covers, heroes, and logos from SteamGridDB — your library looks 100× better."
            badge={
              options.download_art && options.steamgriddb_api_key.trim() ? (
                <Tag color="green">KEY SET</Tag>
              ) : null
            }
          />
          <ToggleRow
            checked={options.remove_missing}
            onChange={(v) => set({ remove_missing: v })}
            title="Prune missing shortcuts"
            description="Remove Steam shortcuts whose source games are gone."
          />
          <ToggleRow
            checked={options.use_uri}
            onChange={(v) => set({ use_uri: v })}
            title="Launch via launcher URI"
            description="Needed for some online games like GTAV that won't run without their launcher."
          />
        </Space>
      </section>

      <section className="ss-panel">
        <Collapse
          ghost
          items={[
            {
              key: "advanced",
              label: (
                <span
                  style={{
                    fontFamily: '"Rajdhani", Inter, sans-serif',
                    fontWeight: 600,
                    fontSize: 13,
                    letterSpacing: "0.16em",
                    textTransform: "uppercase",
                    color: "var(--ss-text-muted)",
                  }}
                >
                  <InfoCircleOutlined /> Advanced settings
                </span>
              ),
              children: (
                <Form layout="vertical" style={{ marginTop: 4 }}>
                  <Form.Item
                    label="Steam install path"
                    help="Auto-detected. Change only if you know what you're doing."
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
                </Form>
              ),
            },
          ]}
        />
      </section>

      <Button
        type="primary"
        size="large"
        block
        disabled={!options.steamid || selectedCount === 0}
        onClick={onProceed}
      >
        Continue with {selectedCount} of {totalGames} games →
      </Button>

      <Modal
        open={showSgdbModal}
        title={
          <Space>
            <PictureOutlined style={{ color: "var(--ss-accent)" }} />
            <span>Cover art set-up</span>
          </Space>
        }
        onCancel={() => setShowSgdbModal(false)}
        footer={null}
        width={620}
        centered
      >
        <Paragraph>
          Cover art comes from <Text strong>SteamGridDB</Text> — a free
          community-curated database. Takes about a minute to set up, and you
          only have to do it once.
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

        <Form.Item label="API key" style={{ marginTop: 16 }}>
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
    </Space>
  );
}
