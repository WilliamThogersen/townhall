import { useCallback, useState, type ReactNode } from "react";
import { DaemonBanner } from "./DaemonBanner";
import { useGlobalKeys } from "./globalKeys";
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
  useGlobalKeys(openHelp);
  return (
    <div className="th-shell" data-view="main">
      <Sidebar foot={sidebarFoot} />
      <main className="th-main">
        <DaemonBanner />
        {children}
      </main>
      <KeyBar />
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
