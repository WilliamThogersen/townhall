import type { ReactNode } from "react";
import { AgentsProvider } from "./agents/AgentsProvider";
import { DataProvider } from "./data";
import { useRoute } from "./router";
import { useLoadSettings } from "./settings/useSettings";
import { Shell } from "./shell/Shell";
import { ControlCenterPage } from "./views/control/ControlCenterPage";
import { ConversationPage } from "./views/conversation/ConversationPage";
import { HomePage } from "./views/home/HomePage";
import { NotFound } from "./views/NotFound";
import { ReposPage } from "./views/repos/ReposPage";
import { SearchPage } from "./views/search/SearchPage";
import { SettingsPage } from "./views/settings/SettingsPage";

function Screen() {
  const route = useRoute();
  switch (route.name) {
    case "home":
      return <HomePage />;
    case "conversation":
      return <ConversationPage id={route.params.id ?? ""} />;
    case "control":
      return <ControlCenterPage />;
    case "repos":
      return <ReposPage />;
    case "settings":
      return <SettingsPage />;
    case "search":
      return <SearchPage />;
    case "not-found":
      return <NotFound path={route.path} />;
  }
}

export function App({ sidebarFoot }: { readonly sidebarFoot?: ReactNode }) {
  useLoadSettings();
  return (
    <DataProvider>
      <AgentsProvider>
        <Shell sidebarFoot={sidebarFoot}>
          <Screen />
        </Shell>
      </AgentsProvider>
    </DataProvider>
  );
}
