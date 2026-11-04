import { AGENT_STATE_ORDER, StatusGlyph, type AgentState } from "../../../ui/StatusGlyph";
import type { TaskGlyphState } from "./taskRows";

const AGENT_STATES: ReadonlySet<string> = new Set(AGENT_STATE_ORDER);

function isAgentState(state: TaskGlyphState): state is AgentState {
  return AGENT_STATES.has(state);
}

export function TaskGlyph({ state }: { readonly state: TaskGlyphState }) {
  if (isAgentState(state)) return <StatusGlyph state={state} />;
  return <span className="th-status" data-state={state} aria-hidden="true" />;
}
