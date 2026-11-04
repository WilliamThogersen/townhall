import type { ReactNode } from "react";
import { DaemonBanner } from "./DaemonBanner";
import { useGlobalKeys } from "./globalKeys";
import { KeyBar } from "./KeyBar";
import { KeyHintsProvider } from "./keyHints";
import { Sidebar } from "./Sidebar";

export interface ShellProps {
  readonly children: ReactNode;
  readonly sidebarFoot?: ReactNode;
}

function Frame({ children, sidebarFoot }: ShellProps) {
  useGlobalKeys();
  return (
    <div className="th-shell" data-view="main">
      <Sidebar foot={sidebarFoot} />
      <main className="th-main">
        <DaemonBanner />
        {children}
      </main>
      <KeyBar />
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
