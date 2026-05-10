import { useState } from "react";
import { Layout, Tabs, Typography } from "antd";
import DetectView from "./views/DetectView";
import ConfigureView from "./views/ConfigureView";
import ApplyView from "./views/ApplyView";
import type { Game, SteamAccount, SyncOptions } from "./types";

const { Header, Content } = Layout;
const { Title } = Typography;

const DEFAULT_OPTIONS: SyncOptions = {
  steamid: "",
  sources: ["epicstore", "xbox"],
  use_uri: false,
  replace_existing: false,
  remove_missing: false,
  download_art: false,
  egs_manifests: "C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\Manifests",
  steam_path: "C:\\Program Files (x86)\\Steam",
  steamgriddb_api_key: "",
};

export default function App() {
  const [games, setGames] = useState<Game[]>([]);
  const [accounts, setAccounts] = useState<SteamAccount[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [options, setOptions] = useState<SyncOptions>(DEFAULT_OPTIONS);
  const [activeTab, setActiveTab] = useState("detect");

  return (
    <Layout style={{ minHeight: "100vh" }}>
      <Header>
        <Title level={3} style={{ color: "white", margin: "16px 0" }}>
          steamsync
        </Title>
      </Header>
      <Content style={{ padding: 24 }}>
        <Tabs
          activeKey={activeTab}
          onChange={setActiveTab}
          items={[
            {
              key: "detect",
              label: "1. Detect",
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
              label: "2. Configure",
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
              label: "3. Apply",
              disabled: games.length === 0 || !options.steamid,
              children: (
                <ApplyView
                  options={options}
                  selectedAppNames={Array.from(selected)}
                />
              ),
            },
          ]}
        />
      </Content>
    </Layout>
  );
}
