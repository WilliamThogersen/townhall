import type { ReactNode } from "react";
import { AgentsProvider } from "./agents/AgentsProvider";
import { useRoute } from "./router";
import { Shell } from "./shell/Shell";
import { LegacyControlCenter } from "./views/LegacyControlCenter";
import { ReposPage } from "./views/repos/ReposPage";
import { SettingsPage } from "./views/settings/SettingsPage";
import {
  ConversationPlaceholder,
  HomePlaceholder,
  NotFound,
  SearchPlaceholder,
} from "./views/placeholders";

function Screen() {
  const route = useRoute();
  switch (route.name) {
    case "home":
      return <HomePlaceholder />;
    case "conversation":
      return <ConversationPlaceholder id={route.params.id ?? ""} />;
    case "control":
      return <LegacyControlCenter />;
    case "repos":
      return <ReposPage />;
    case "settings":
      return <SettingsPage />;
    case "search":
      return <SearchPlaceholder />;
    case "not-found":
      return <NotFound path={route.path} />;
  }
}

export function App({ sidebarFoot }: { readonly sidebarFoot?: ReactNode }) {
  return (
    <AgentsProvider>
      <Shell sidebarFoot={sidebarFoot}>
        <Screen />
      </Shell>
    </AgentsProvider>
  );
}
