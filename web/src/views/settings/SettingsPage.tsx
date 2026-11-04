import { useSettings, type SettingsControls } from "../../settings/useSettings";
import { useKeyHints, type KeyHint } from "../../shell/keyHints";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { AgentsGroup, AppearanceGroup, ConversationsGroup } from "./SettingGroups";
import "./cap.css";

export const SETTINGS_HINTS: readonly KeyHint[] = [
  { keys: "tab", label: "next setting" },
  { keys: "← →", label: "change choice" },
];

type ProblemProps = Pick<SettingsControls, "problem" | "loading" | "retry" | "dismissProblem">;

function SettingsProblem({ problem, loading, retry, dismissProblem }: ProblemProps) {
  if (problem === null) return null;
  const action =
    problem.kind === "load" ? (
      <Button size="sm" busy={loading} busyLabel="retrying…" onClick={retry}>
        retry
      </Button>
    ) : (
      <Button size="sm" onClick={dismissProblem}>
        dismiss
      </Button>
    );
  return (
    <Banner level="error" action={action}>
      {problem.message}
    </Banner>
  );
}

export function SettingsPage() {
  useKeyHints(SETTINGS_HINTS);
  const { problem, loading, retry, dismissProblem } = useSettings();
  return (
    <div className="th-page">
      <header className="th-page-head">
        <h1>settings</h1>
        <span className="th-cap">saved as you change them</span>
      </header>
      <div className="th-page-body">
        <SettingsProblem problem={problem} loading={loading} retry={retry} dismissProblem={dismissProblem} />
        <div className="th-settings" aria-busy={loading}>
          <AgentsGroup />
          <ConversationsGroup />
          <AppearanceGroup />
        </div>
      </div>
    </div>
  );
}
