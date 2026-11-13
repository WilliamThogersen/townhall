import type { ReactNode } from "react";
import { AgentsProvider } from "./agents/AgentsProvider";
import { DataProvider } from "./data";
import { useRoute } from "./router";
import { useLoadSettings } from "./settings/useSettings";
import { Shell } from "./shell/Shell";
import { TaskNotifier } from "./tasks/TaskNotifier";
import { PluginTheme } from "./theme/pluginTheme";
import { ToastProvider } from "./ui/Toasts";
import { AgentsPage } from "./views/agents/AgentsPage";
import { ControlCenterPage } from "./views/control/ControlCenterPage";
import { ConversationPage } from "./views/conversation/ConversationPage";
import { HomePage } from "./views/home/HomePage";
import { NotFound } from "./views/NotFound";
import { PluginPage } from "./views/plugins/PluginPage";
import { PluginsPage } from "./views/plugins/PluginsPage";
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
    case "agents":
      return <AgentsPage />;
    case "plugins":
      return <PluginsPage />;
    case "settings":
      return <SettingsPage />;
    case "search":
      return <SearchPage />;
    case "plugin-page":
      return <PluginPage plugin={route.params.plugin ?? ""} page={route.params.page ?? ""} />;
    case "not-found":
      return <NotFound path={route.path} />;
  }
}

export function App({ sidebarFoot }: { readonly sidebarFoot?: ReactNode }) {
  useLoadSettings();
  return (
    <DataProvider>
      <PluginTheme />
      <TaskNotifier />
      <AgentsProvider>
        <ToastProvider>
          <Shell sidebarFoot={sidebarFoot}>
            <Screen />
          </Shell>
        </ToastProvider>
      </AgentsProvider>
    </DataProvider>
  );
}
