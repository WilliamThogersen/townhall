import type { ReactNode } from "react";
import { useAgentsContext } from "../agents/AgentsProvider";
import { countByState } from "../agents/agentState";
import { Link, PATHS, useRoute } from "../router";
import { GlyphCounts } from "../ui/StatusGlyph";
import { Kbd } from "../ui/Kbd";
import { Logo } from "../ui/Logo";
import { useRepoCount } from "./useRepoCount";
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

function Nav() {
  const { name } = useRoute();
  const { agents, loading, error } = useAgentsContext();
  const repos = useRepoCount(!loading && error === null);
  return (
    <div className="th-nav">
      <NavItem to={PATHS.home} icon="+" label="new conversation" current={name === "home"} shortcut="n" />
      <NavItem to={PATHS.search} icon="/" label="search" current={name === "search"} shortcut="/" />
      <NavItem to={PATHS.repos} icon="~" label="repos" current={name === "repos"}>
        {repos !== null && <span className="th-nav-count">{repos}</span>}
      </NavItem>
      <NavItem to={PATHS.control} icon="$" label="control center" current={name === "control"}>
        <GlyphCounts counts={countByState(agents)} className="th-nav-count" />
      </NavItem>
    </div>
  );
}

export function Sidebar({ foot }: { readonly foot?: ReactNode }) {
  const { name } = useRoute();
  return (
    <nav className="th-sidebar" aria-label="townhall">
      <Link className="th-sidebar-top" to={PATHS.control} title="control center" aria-current={currentPage(name === "control")}>
        <Logo />
        <span className="th-wordmark">townhall</span>
      </Link>
      <Nav />
      <div className="th-sidebar-scroll">
        <h3 className="th-section">active 0</h3>
        <p className="th-sidebar-note">no conversations yet</p>
        <h3 className="th-section">older 0</h3>
      </div>
      <div className="th-sidebar-foot">
        {foot}
        <Link className="th-btn th-btn--ghost th-btn--sm" to={PATHS.settings} aria-current={currentPage(name === "settings")}>
          settings
        </Link>
      </div>
    </nav>
  );
}
