import { isDesktop } from "../../api/base";
import { useRepo } from "../../desktop/useRepo";
import { useInfo } from "../../repos/useInfo";
import { NEW_CONVERSATION_HINT, SEARCH_HINT } from "../../shell/globalKeys";
import { useKeyHints } from "../../shell/keyHints";
import { Button } from "../../ui/Button";
import { ReposView } from "./ReposView";

export const BROWSER_NOTE = "info: the daemon serves one repo. start it with another path to switch";
export const EXTERNAL_NOTE = "info: a daemon started outside the app serves this repo. start it with another path to switch";

function HeadNote({ text }: { readonly text: string }) {
  return (
    <p className="th-cap th-repos-note" title={text}>
      {text}
    </p>
  );
}

function DesktopRepos() {
  const repo = useInfo();
  const desktop = useRepo();
  const change = async () => {
    await desktop.choose();
    repo.reload();
  };
  const action =
    desktop.status?.mode === "external" ? (
      <HeadNote text={EXTERNAL_NOTE} />
    ) : (
      <div className="th-page-head-actions">
        <Button variant="primary" size="sm" busy={desktop.busy} busyLabel="choosing…" onClick={() => void change()}>
          change repository…
        </Button>
      </div>
    );
  return <ReposView repo={repo} action={action} problem={desktop.error} />;
}

function BrowserRepos() {
  const repo = useInfo();
  return <ReposView repo={repo} action={<HeadNote text={BROWSER_NOTE} />} />;
}

const REPOS_HINTS = [NEW_CONVERSATION_HINT, SEARCH_HINT];

export function ReposPage() {
  useKeyHints(REPOS_HINTS);
  return isDesktop() ? <DesktopRepos /> : <BrowserRepos />;
}
