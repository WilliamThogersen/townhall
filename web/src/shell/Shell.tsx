import { useCallback, useState, type ReactNode } from "react";
import { DaemonBanner } from "./DaemonBanner";
import { MENU_HINT, useGlobalKeys } from "./globalKeys";
import { useMenuDrawer } from "./useMenuDrawer";
import { HelpOverlay } from "./HelpOverlay";
import { KeyBar } from "./KeyBar";
import { KeyHintsProvider } from "./keyHints";
import { Sidebar } from "./Sidebar";

export interface ShellProps {
  readonly children: ReactNode;
  readonly sidebarFoot?: ReactNode;
}

function Frame({ children, sidebarFoot }: ShellProps) {
  const [helpOpen, setHelpOpen] = useState(false);
  const openHelp = useCallback(() => setHelpOpen(true), []);
  const closeHelp = useCallback(() => setHelpOpen(false), []);
  const menu = useMenuDrawer();
  useGlobalKeys(openHelp, menu.toggle);
  return (
    <div className="th-shell" data-view={menu.open ? "side" : "main"}>
      <Sidebar foot={sidebarFoot} />
      <main className="th-main">
        <DaemonBanner />
        {children}
      </main>
      <KeyBar />
      <header className="th-navbar">
        <button
          type="button"
          className="th-btn th-btn--ghost th-btn--sm"
          aria-expanded={menu.open}
          aria-keyshortcuts={MENU_HINT.keys}
          onClick={menu.toggle}
        >
          menu
        </button>
        <span className="th-wordmark">townhall</span>
      </header>
      {helpOpen ? <HelpOverlay onClose={closeHelp} /> : null}
    </div>
  );
}

export function Shell(props: ShellProps) {
  return (
    <KeyHintsProvider>
      <Frame {...props} />
    </KeyHintsProvider>
  );
}
