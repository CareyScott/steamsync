import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Button,
  ConfigProvider,
  Layout,
  Modal,
  Space,
  Tabs,
  Tooltip,
  Typography,
  theme as antdTheme,
} from "antd";
import {
  BulbFilled,
  BulbOutlined,
  CheckCircleOutlined,
  RocketOutlined,
} from "@ant-design/icons";
import DetectView from "./views/DetectView";
import ConfigureView from "./views/ConfigureView";
import ApplyView from "./views/ApplyView";
import type { Game, SteamAccount, SyncOptions } from "./types";
import { autoDetectSteamPath } from "./api";

const { Header, Content } = Layout;
const { Title, Paragraph, Text } = Typography;

const DEFAULT_OPTIONS: SyncOptions = {
  steamid: "",
  sources: ["epicstore", "xbox"],
  use_uri: false,
  replace_existing: false,
  remove_missing: false,
  download_art: false,
  egs_manifests: "C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\Manifests",
  steam_path: "",
  steamgriddb_api_key: "",
};

const STORAGE = {
  OPTIONS: "steamsync.options.v1",
  THEME: "steamsync.theme.v1",
  SEEN_WELCOME: "steamsync.seenWelcome.v1",
};

/** Read JSON from localStorage, falling back to a default on any error. */
function loadStored<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw);
    return { ...fallback, ...parsed } as T;
  } catch {
    return fallback;
  }
}

export default function App() {
  // Theme persists across launches.
  const [themeMode, setThemeMode] = useState<"dark" | "light">(() => {
    const stored = localStorage.getItem(STORAGE.THEME);
    return stored === "light" ? "light" : "dark";
  });

  // Settings persist across launches. We load synchronously so the first
  // render already has the user's preferences.
  const [options, setOptionsRaw] = useState<SyncOptions>(() =>
    loadStored<SyncOptions>(STORAGE.OPTIONS, DEFAULT_OPTIONS),
  );

  const setOptions = useCallback((next: SyncOptions) => {
    setOptionsRaw(next);
    try {
      localStorage.setItem(STORAGE.OPTIONS, JSON.stringify(next));
    } catch {
      // Out of quota / blocked storage — silent fall-through is fine.
    }
  }, []);

  // First-run welcome card.
  const [showWelcome, setShowWelcome] = useState<boolean>(
    () => localStorage.getItem(STORAGE.SEEN_WELCOME) !== "true",
  );

  // Auto-detect Steam path once on mount if the user hasn't set one.
  useEffect(() => {
    if (options.steam_path) return;
    autoDetectSteamPath()
      .then((path) => {
        if (path) setOptions({ ...options, steam_path: path });
      })
      .catch(() => {
        /* registry probe failed — UI keeps the field blank, user can fill in */
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    localStorage.setItem(STORAGE.THEME, themeMode);
  }, [themeMode]);

  const [games, setGames] = useState<Game[]>([]);
  const [accounts, setAccounts] = useState<SteamAccount[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [activeTab, setActiveTab] = useState("detect");

  const selectedGames = useMemo(
    () => games.filter((g) => selected.has(g.app_name)),
    [games, selected],
  );

  const dismissWelcome = () => {
    localStorage.setItem(STORAGE.SEEN_WELCOME, "true");
    setShowWelcome(false);
  };

  const themeConfig = useMemo(
    () => ({
      algorithm:
        themeMode === "dark"
          ? antdTheme.darkAlgorithm
          : antdTheme.defaultAlgorithm,
      token: {
        colorPrimary: "#5b6cff",
        borderRadius: 8,
      },
    }),
    [themeMode],
  );

  return (
    <ConfigProvider theme={themeConfig}>
      <Layout style={{ minHeight: "100vh" }}>
        <Header
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            background: themeMode === "dark" ? "#141426" : "#fafbff",
            borderBottom:
              themeMode === "dark" ? "1px solid #20203a" : "1px solid #e5e7f0",
            padding: "0 24px",
          }}
        >
          <Space size="middle" align="center">
            <RocketOutlined
              style={{ fontSize: 22, color: "#5b6cff" }}
            />
            <Title
              level={4}
              style={{
                margin: 0,
                color: themeMode === "dark" ? "#fff" : "#1d1d1f",
              }}
            >
              steamsync
            </Title>
            <Text type="secondary" style={{ fontSize: 12 }}>
              Add Epic &amp; Xbox games to Steam
            </Text>
          </Space>
          <Tooltip title={themeMode === "dark" ? "Switch to light" : "Switch to dark"}>
            <Button
              type="text"
              shape="circle"
              icon={themeMode === "dark" ? <BulbOutlined /> : <BulbFilled />}
              onClick={() =>
                setThemeMode(themeMode === "dark" ? "light" : "dark")
              }
            />
          </Tooltip>
        </Header>

        <Content style={{ padding: 24, maxWidth: 1200, width: "100%", margin: "0 auto" }}>
          <Tabs
            activeKey={activeTab}
            onChange={setActiveTab}
            items={[
              {
                key: "detect",
                label: "1. Find games",
                children: (
                  <DetectView
                    options={options}
                    onOptionsChange={setOptions}
                    games={games}
                    setGames={setGames}
                    accounts={accounts}
                    setAccounts={setAccounts}
                    selected={selected}
                    setSelected={setSelected}
                    onProceed={() => setActiveTab("configure")}
                  />
                ),
              },
              {
                key: "configure",
                label: "2. Choose options",
                disabled: games.length === 0,
                children: (
                  <ConfigureView
                    options={options}
                    onOptionsChange={setOptions}
                    accounts={accounts}
                    selectedCount={selected.size}
                    totalGames={games.length}
                    onProceed={() => setActiveTab("apply")}
                  />
                ),
              },
              {
                key: "apply",
                label: "3. Add to Steam",
                disabled: games.length === 0 || !options.steamid,
                children: (
                  <ApplyView
                    options={options}
                    selectedGames={selectedGames}
                    onSuccess={() => {
                      // After a successful apply, clear selection so the
                      // next run starts fresh.
                      setSelected(new Set());
                    }}
                  />
                ),
              },
            ]}
          />
        </Content>

        <Modal
          open={showWelcome}
          title={
            <Space>
              <RocketOutlined style={{ color: "#5b6cff" }} />
              <span>Welcome to steamsync</span>
            </Space>
          }
          okText="Get started"
          cancelButtonProps={{ style: { display: "none" } }}
          onOk={dismissWelcome}
          onCancel={dismissWelcome}
          width={560}
          centered
        >
          <Paragraph>
            steamsync finds the games you've installed from Epic Games Store and
            the Xbox app, then adds them to Steam as shortcuts — so your whole
            library is in one place.
          </Paragraph>
          <Paragraph>
            <CheckCircleOutlined style={{ color: "#52c41a", marginRight: 6 }} />
            <Text strong>Safe by default.</Text> Your Steam shortcuts file is
            backed up before any change.
          </Paragraph>
          <Paragraph>
            <CheckCircleOutlined style={{ color: "#52c41a", marginRight: 6 }} />
            <Text strong>No account needed</Text> for basic syncing. (Optional:
            link a free SteamGridDB account to also pull cover art.)
          </Paragraph>
          <Paragraph type="secondary" style={{ fontSize: 12, marginBottom: 0 }}>
            Three steps: find your games → choose which ones to add → add them.
            Restart Steam after.
          </Paragraph>
        </Modal>
      </Layout>
    </ConfigProvider>
  );
}
