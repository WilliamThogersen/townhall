import { useCallback, useState } from "react";
import { toMessage } from "../../api/errors";
import { usePlugins } from "../../data";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { DRAG_REGION } from "../../ui/dragRegion";
import { Loading } from "../../ui/Loading";
import { AvailableSection } from "./AvailableSection";
import { InstalledSection } from "./InstalledSection";
import { MarketplaceSection } from "./MarketplaceSection";
import { PermissionPrompt } from "./PermissionPrompt";
import { useInstallFlow, type Run } from "./useInstallFlow";
import "../settings/cap.css";
import "./plugins.css";

function useRun(): { readonly problem: string | null; readonly dismiss: () => void; readonly run: Run } {
  const [problem, setProblem] = useState<string | null>(null);
  const run = useCallback<Run>(async (action) => {
    setProblem(null);
    try {
      return await action();
    } catch (e) {
      setProblem(toMessage(e));
      return undefined;
    }
  }, []);
  const dismiss = useCallback(() => setProblem(null), []);
  return { problem, dismiss, run };
}

export function PluginsPage() {
  const plugins = usePlugins();
  const { problem, dismiss, run } = useRun();
  const flow = useInstallFlow(plugins, run);
  const marketName = flow.id?.slice(flow.id.lastIndexOf("@") + 1).toLowerCase();
  const official = plugins.marketplaces.some((m) => m.official && m.name.toLowerCase() === marketName);
  const waiting = plugins.loading && plugins.marketplaces.length === 0 && plugins.error === null;
  return (
    <div className="th-page">
      <header className="th-page-head" {...DRAG_REGION}>
        <h1>plugins</h1>
        <span className="th-cap">{plugins.installed.length} installed</span>
      </header>
      <div className="th-page-body">
        {problem === null ? null : (
          <Banner
            level="error"
            action={
              <Button size="sm" onClick={dismiss}>
                dismiss
              </Button>
            }
          >
            {problem}
          </Banner>
        )}
        {plugins.error === null ? null : <Banner level="error">{`plugins not loaded: ${plugins.error}`}</Banner>}
        {waiting ? (
          <div className="th-empty">
            <Loading verb="loading plugins" />
          </div>
        ) : (
          <>
            <MarketplaceSection plugins={plugins} run={run} />
            <InstalledSection plugins={plugins} run={run} />
            <AvailableSection marketplaces={plugins.marketplaces} available={plugins.available} onInstall={flow.begin} />
          </>
        )}
      </div>
      {flow.id === null ? null : (
        <PermissionPrompt
          id={flow.id}
          official={official}
          preview={flow.preview}
          error={flow.error}
          busy={flow.busy}
          onInstall={flow.install}
          onCancel={flow.cancel}
        />
      )}
    </div>
  );
}
