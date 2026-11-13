import type { ReactNode } from "react";
import { useAgentsContext } from "../agents/AgentsProvider";
import { countByState } from "../agents/agentState";
import { useRepos } from "../data";
import { PluginNavItems } from "../plugins/PluginNavItems";
import { PluginSlot } from "../plugins/PluginSlot";
import { Link, PATHS, useRoute } from "../router";
import { ProgressOverview } from "../sidebar/ProgressOverview";
import { SearchNavItem } from "../sidebar/SearchNavItem";
import { SidebarConversations } from "../sidebar/SidebarConversations";
import { UsageMeter } from "../usage/UsageMeter";
import { useSidebarSearch, type SidebarSearch } from "../sidebar/useSidebarSearch";
import { DRAG_REGION } from "../ui/dragRegion";
import { GlyphCounts } from "../ui/StatusGlyph";
import { Kbd } from "../ui/Kbd";
import { Logo } from "../ui/Logo";
import { NEW_CONVERSATION_HINT } from "./globalKeys";
import "./shell.css";

interface NavItemProps {
  readonly to: string;
  readonly icon: string;
  readonly label: string;
  readonly current: boolean;
  readonly shortcut?: string;
  readonly children?: ReactNode;
}

const currentPage = (current: boolean) => (current ? "page" : undefined);

function NavItem({ to, icon, label, current, shortcut, children }: NavItemProps) {
  return (
    <Link className="th-nav-item" to={to} aria-current={currentPage(current)} aria-keyshortcuts={shortcut}>
      <span className="th-nav-icon" aria-hidden="true">{icon}</span>
      <span>{label}</span>
      {shortcut ? <span aria-hidden="true"><Kbd>{shortcut}</Kbd></span> : children}
    </Link>
  );
}

function Nav({ search }: { readonly search: SidebarSearch }) {
  const { name } = useRoute();
  const { agents } = useAgentsContext();
  const { repos } = useRepos();
  return (
    <div className="th-nav">
      <NavItem to={PATHS.home} icon="+" label="new conversation" current={name === "home"} shortcut={NEW_CONVERSATION_HINT.keys} />
      <SearchNavItem search={search} current={name === "search"} />
      <NavItem to={PATHS.repos} icon="~" label="repos" current={name === "repos"}>
        {repos !== null && <span className="th-nav-count">{repos.length}</span>}
      </NavItem>
      <NavItem to={PATHS.agents} icon="@" label="agents" current={name === "agents"} />
      <NavItem to={PATHS.plugins} icon="&" label="plugins" current={name === "plugins"} />
      <NavItem to={PATHS.control} icon="$" label="control center" current={name === "control"}>
        <GlyphCounts counts={countByState(agents)} className="th-nav-count" />
      </NavItem>
      <PluginNavItems />
    </div>
  );
}

export function Sidebar({ foot }: { readonly foot?: ReactNode }) {
  const { name } = useRoute();
  const search = useSidebarSearch();
  return (
    <nav className="th-sidebar" aria-label="townhall">
      <div className="th-sidebar-top" {...DRAG_REGION}>
        <Link className="th-sidebar-brand" to={PATHS.control} title="control center" aria-current={currentPage(name === "control")}>
          <Logo />
          <span className="th-wordmark">townhall</span>
        </Link>
      </div>
      <Nav search={search} />
      <ProgressOverview />
      <SidebarConversations query={search.query} />
      <div className="th-sidebar-foot">
        <UsageMeter />
        <PluginSlot name="sidebar.foot" />
        {foot}
        <Link className="th-btn th-btn--ghost th-btn--sm" to={PATHS.settings} aria-current={currentPage(name === "settings")}>
          settings
        </Link>
      </div>
    </nav>
  );
}
