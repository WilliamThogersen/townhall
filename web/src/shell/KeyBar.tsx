import { useAgentsContext } from "../agents/AgentsProvider";
import { countByState } from "../agents/agentState";
import { Kbd } from "../ui/Kbd";
import { GlyphCounts } from "../ui/StatusGlyph";
import { HELP_HINT } from "./globalKeys";
import { useCurrentKeyHints } from "./keyHints";
import { SystemStats } from "./SystemStats";

export function KeyBar() {
  const hints = useCurrentKeyHints();
  const { agents } = useAgentsContext();
  return (
    <footer className="th-keybar">
      {hints.map((hint) => (
        <span key={`${hint.keys} ${hint.label}`}>
          <Kbd>{hint.keys}</Kbd>
          {hint.label}
        </span>
      ))}
      <span>
        <Kbd>{HELP_HINT.keys}</Kbd>
        {HELP_HINT.label}
      </span>
      <span className="th-keybar-status">
        <SystemStats />
        <GlyphCounts counts={countByState(agents)} labelled />
      </span>
    </footer>
  );
}
