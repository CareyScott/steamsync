import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ConfigProvider,
  Modal,
  Space,
  Tooltip,
  Typography,
  theme as antdTheme,
} from "antd";
import { BulbFilled, BulbOutlined, CheckCircleOutlined } from "@ant-design/icons";
import DetectView from "./views/DetectView";
import ConfigureView from "./views/ConfigureView";
import ApplyView from "./views/ApplyView";
import type { Game, SteamAccount, SyncOptions } from "./types";
import { autoDetectSteamPath } from "./api";

const { Paragraph, Text } = Typography;

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
  local_folders: [],
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

type TabKey = "detect" | "configure" | "apply";

const STEPS: { key: TabKey; label: string; hint: string }[] = [
  { key: "detect", label: "Scan", hint: "Find games" },
  { key: "configure", label: "Configure", hint: "Choose options" },
  { key: "apply", label: "Deploy", hint: "Add to Steam" },
];

function Logo() {
  // Hexagonal mark with an inset play/sync glyph — original, not Steam's logo.
  return (
    <svg width="36" height="36" viewBox="0 0 40 40" fill="none" xmlns="http://www.w3.org/2000/svg">
      <defs>
        <linearGradient id="ss-hex" x1="0" y1="0" x2="40" y2="40">
          <stop offset="0%" stopColor="#66c0f4" />
          <stop offset="100%" stopColor="#1a6c9c" />
        </linearGradient>
        <linearGradient id="ss-glow" x1="0" y1="0" x2="0" y2="40">
          <stop offset="0%" stopColor="#a4d007" />
          <stop offset="100%" stopColor="#66c0f4" />
        </linearGradient>
      </defs>
      <path
        d="M20 2 L36 11 L36 29 L20 38 L4 29 L4 11 Z"
        fill="url(#ss-hex)"
        stroke="#66c0f4"
        strokeWidth="1.2"
        opacity="0.95"
      />
      <path
        d="M20 6 L32 13 L32 27 L20 34 L8 27 L8 13 Z"
        fill="#0a1118"
        opacity="0.55"
      />
      {/* S/sync glyph */}
      <path
        d="M15 14 Q22 14 22 19 Q22 22 18 22 L22 22 Q25 22 25 25 Q25 30 18 30"
        stroke="url(#ss-glow)"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
      <circle cx="15" cy="14" r="1.6" fill="#a4d007" />
      <circle cx="25" cy="30" r="1.6" fill="#66c0f4" />
    </svg>
  );
}

export default function App() {
  const [themeMode, setThemeMode] = useState<"dark" | "light">(() => {
    const stored = localStorage.getItem(STORAGE.THEME);
    return stored === "light" ? "light" : "dark";
  });

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

  const [showWelcome, setShowWelcome] = useState<boolean>(
    () => localStorage.getItem(STORAGE.SEEN_WELCOME) !== "true",
  );

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
    document.documentElement.setAttribute("data-theme", themeMode);
  }, [themeMode]);

  const [games, setGames] = useState<Game[]>([]);
  const [accounts, setAccounts] = useState<SteamAccount[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [exeOverrides, setExeOverrides] = useState<Record<string, string>>({});
  const [activeTab, setActiveTab] = useState<TabKey>("detect");

  const selectedGames = useMemo(
    () => games.filter((g) => selected.has(g.app_name)),
    [games, selected],
  );

  // Reset the Apply view back to its idle state once the user leaves
  // it *after* a successful run — see the original comment for full detail.
  const [applyResetKey, setApplyResetKey] = useState(0);
  const [applyDidSucceed, setApplyDidSucceed] = useState(false);
  const prevTabRef = useRef(activeTab);
  useEffect(() => {
    if (
      prevTabRef.current === "apply" &&
      activeTab !== "apply" &&
      applyDidSucceed
    ) {
      setApplyResetKey((k) => k + 1);
      setApplyDidSucceed(false);
    }
    prevTabRef.current = activeTab;
  }, [activeTab, applyDidSucceed]);

  const dismissWelcome = () => {
    localStorage.setItem(STORAGE.SEEN_WELCOME, "true");
    setShowWelcome(false);
  };

  const isStepDisabled = (key: TabKey) => {
    if (key === "configure") return games.length === 0;
    if (key === "apply") return games.length === 0 || !options.steamid;
    return false;
  };

  const stepIndex = STEPS.findIndex((s) => s.key === activeTab);

  const themeConfig = useMemo(
    () => ({
      algorithm:
        themeMode === "dark"
          ? antdTheme.darkAlgorithm
          : antdTheme.defaultAlgorithm,
      token: {
        colorPrimary: "#66c0f4",
        colorInfo: "#66c0f4",
        colorSuccess: "#a4d007",
        colorWarning: "#e0a225",
        colorError: "#d94126",
        colorBgBase: themeMode === "dark" ? "#0a1118" : "#eef2f7",
        colorBgContainer: themeMode === "dark" ? "#131e29" : "#ffffff",
        colorBgElevated: themeMode === "dark" ? "#1b2838" : "#f6f8fb",
        colorBorder: themeMode === "dark" ? "#2a3f5a" : "#d6dde6",
        colorBorderSecondary: themeMode === "dark" ? "#1f3047" : "#e3e8ee",
        colorText: themeMode === "dark" ? "#c7d5e0" : "#1b2838",
        colorTextSecondary: themeMode === "dark" ? "#8b9bab" : "#4a5a6a",
        colorTextTertiary: themeMode === "dark" ? "#67788a" : "#67788a",
        borderRadius: 4,
        borderRadiusLG: 6,
        borderRadiusSM: 3,
        fontFamily: '"Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
      },
    }),
    [themeMode],
  );

  return (
    <ConfigProvider theme={themeConfig}>
      <div className="ss-shell">
        <header className="ss-header">
          <div className="ss-brand">
            <div className="ss-logo">
              <Logo />
            </div>
            <div className="ss-wordmark">
              <div className="name">
                steam<span className="accent">sync</span>
              </div>
              <div className="tagline">Unify · Library · One Click</div>
            </div>
          </div>
          <div className="ss-header-right">
            <Tooltip title={themeMode === "dark" ? "Switch to light" : "Switch to dark"}>
              <button
                type="button"
                className="ss-icon-btn"
                onClick={() =>
                  setThemeMode(themeMode === "dark" ? "light" : "dark")
                }
                aria-label="Toggle theme"
              >
                {themeMode === "dark" ? <BulbOutlined /> : <BulbFilled />}
              </button>
            </Tooltip>
          </div>
        </header>

        <main className="ss-content">
          <nav className="ss-steps" aria-label="Workflow steps">
            {STEPS.map((step, i) => {
              const disabled = isStepDisabled(step.key);
              const active = step.key === activeTab;
              const complete = i < stepIndex && !disabled;
              return (
                <button
                  key={step.key}
                  type="button"
                  className={[
                    "ss-step",
                    active ? "active" : "",
                    complete ? "complete" : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  onClick={() => !disabled && setActiveTab(step.key)}
                  disabled={disabled}
                >
                  <span className="ss-step-num">
                    {complete ? <CheckCircleOutlined /> : `0${i + 1}`}
                  </span>
                  <span className="ss-step-text">
                    <span className="ss-step-label">{step.label}</span>
                    <span className="ss-step-hint">{step.hint}</span>
                  </span>
                </button>
              );
            })}
          </nav>

          {activeTab === "detect" && (
            <DetectView
              options={options}
              onOptionsChange={setOptions}
              games={games}
              setGames={setGames}
              accounts={accounts}
              setAccounts={setAccounts}
              selected={selected}
              setSelected={setSelected}
              exeOverrides={exeOverrides}
              setExeOverrides={setExeOverrides}
              onProceed={() => setActiveTab("configure")}
            />
          )}
          {activeTab === "configure" && (
            <ConfigureView
              options={options}
              onOptionsChange={setOptions}
              accounts={accounts}
              selectedCount={selected.size}
              totalGames={games.length}
              onProceed={() => setActiveTab("apply")}
            />
          )}
          {activeTab === "apply" && (
            <ApplyView
              key={applyResetKey}
              options={options}
              selectedGames={selectedGames}
              exeOverrides={exeOverrides}
              onSuccess={() => {
                setSelected(new Set());
                setApplyDidSucceed(true);
              }}
            />
          )}
        </main>

        <footer className="ss-footer">
          steamsync · open source · made for gamers
        </footer>

        <Modal
          open={showWelcome}
          title={
            <Space>
              <span style={{ display: "inline-flex", verticalAlign: "middle" }}>
                <Logo />
              </span>
              <span>Welcome, player</span>
            </Space>
          }
          okText="Let's go"
          cancelButtonProps={{ style: { display: "none" } }}
          onOk={dismissWelcome}
          onCancel={dismissWelcome}
          width={580}
          centered
        >
          <Paragraph style={{ marginBottom: 18 }}>
            steamsync hunts down the games you've installed from Epic and the
            Xbox app and drops them straight into your Steam library as
            shortcuts — so everything lives in one place.
          </Paragraph>
          <div className="ss-welcome-feature">
            <CheckCircleOutlined className="icon" />
            <div>
              <Text strong>Safe by default.</Text>
              <div style={{ fontSize: 13, color: "var(--ss-text-muted)" }}>
                Your Steam shortcuts file is backed up before any change.
              </div>
            </div>
          </div>
          <div className="ss-welcome-feature">
            <CheckCircleOutlined className="icon" />
            <div>
              <Text strong>No account needed.</Text>
              <div style={{ fontSize: 13, color: "var(--ss-text-muted)" }}>
                Optional: link a free SteamGridDB account to pull cover art too.
              </div>
            </div>
          </div>
          <Paragraph
            type="secondary"
            style={{ fontSize: 12, marginTop: 14, marginBottom: 0 }}
          >
            Three steps — scan, configure, deploy. Restart Steam when you're done.
          </Paragraph>
        </Modal>
      </div>
    </ConfigProvider>
  );
}
