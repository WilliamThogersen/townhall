import { useCallback, useEffect, useState } from "react";
import { toMessage } from "../../api/errors";
import {
  acceptSuggestion,
  getPermissions,
  type PermissionDecision,
  type PermissionSuggestion,
  type Permissions,
} from "../../api/permissions";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { SettingRow } from "./SettingRow";
import { HELP } from "./settingsCopy";

const DECISIONS_SHOWN = 8;

interface SuggestionProps {
  readonly suggestion: PermissionSuggestion;
  readonly busy: boolean;
  readonly onAdd: (suggestion: PermissionSuggestion) => void;
}

function Suggestion({ suggestion, busy, onAdd }: SuggestionProps) {
  const { agentType, tool, approvals } = suggestion;
  return (
    <div className="th-suggestion" role="group" aria-label={`Suggestion ${tool} for ${agentType}`}>
      <code className="th-suggestion-tool">{tool}</code>
      <span className="th-suggestion-why">
        allowed {approvals} times for {agentType}
      </span>
      <Button size="sm" disabled={busy} onClick={() => onAdd(suggestion)}>
        add to {agentType}&apos;s tools
      </Button>
    </div>
  );
}

function Decision({ decision }: { readonly decision: PermissionDecision }) {
  return (
    <li className="th-decision" data-verdict={decision.verdict}>
      <span className="th-decision-verdict">{decision.verdict}</span>
      <span className="th-decision-type">{decision.agentType}</span>
      <code className="th-decision-command" title={decision.command}>
        {decision.command}
      </code>
      <span className="th-decision-reason">{decision.reason}</span>
    </li>
  );
}

export function PermissionSuggestions() {
  const [data, setData] = useState<Permissions | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async (signal?: AbortSignal) => {
    try {
      setData(await getPermissions(signal));
      setProblem(null);
    } catch (e) {
      if (signal?.aborted !== true) setProblem(toMessage(e));
    }
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    void load(controller.signal);
    return () => controller.abort();
  }, [load]);

  const add = async (suggestion: PermissionSuggestion) => {
    setBusy(true);
    try {
      await acceptSuggestion(suggestion.agentType, suggestion.tool);
      await load();
    } catch (e) {
      setProblem(`not added: ${toMessage(e)}`);
    } finally {
      setBusy(false);
    }
  };

  const suggestions = data?.suggestions ?? [];
  const decisions = (data?.decisions ?? []).slice(0, DECISIONS_SHOWN);
  return (
    <>
      {problem === null ? null : (
        <div className="th-setting-row" data-wide="true">
          <Banner level="error">{problem}</Banner>
        </div>
      )}
      {suggestions.length === 0 ? null : (
        <SettingRow label="permission suggestions" help={HELP.permissionSuggestions} wide>
          <div className="th-suggestions">
            {suggestions.map((s) => (
              <Suggestion key={`${s.agentType}:${s.tool}`} suggestion={s} busy={busy} onAdd={(x) => void add(x)} />
            ))}
          </div>
        </SettingRow>
      )}
      {decisions.length === 0 ? null : (
        <SettingRow label="recent permission decisions" help={HELP.permissionLog} wide>
          <ul className="th-decisions" aria-label="Recent permission decisions">
            {decisions.map((d) => (
              <Decision key={d.id} decision={d} />
            ))}
          </ul>
        </SettingRow>
      )}
    </>
  );
}
