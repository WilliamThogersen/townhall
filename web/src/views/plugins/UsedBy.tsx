import { useState } from "react";
import { toMessage } from "../../api/errors";
import { getAgentType, saveAgentType } from "../../api/library";
import type { Plugin } from "../../api/types";
import { useAgentTypes } from "../../data";
import { Banner } from "../../ui/Banner";
import { pluginsOf, typesUsing, withPlugin } from "../agents/agentPlugins";
import { isConversationType } from "../agents/agentTypes";
import "./plugins.css";

function usedText(count: number): string {
  if (count === 0) return "used by no agent type";
  return `used by ${count} agent type${count === 1 ? "" : "s"}`;
}

export function UsedBy({ plugin }: { readonly plugin: Plugin }) {
  const { data, error, reload } = useAgentTypes();
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  if (data === null) return <p className="th-cap">{error === null ? "loading agent types" : `agent types not loaded: ${error}`}</p>;
  const using = typesUsing(data, plugin.id);

  const toggle = async (name: string, on: boolean) => {
    setBusy(true);
    setRefused(null);
    try {
      const type = await getAgentType(name);
      await saveAgentType(name, withPlugin(type.source, pluginsOf(type), plugin.id, on));
      reload();
    } catch (e) {
      setRefused(toMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <details className="th-plugin-extra">
      <summary>{usedText(using.length)}</summary>
      {refused === null ? null : <Banner level="error">{refused}</Banner>}
      <ul className="th-agent-plugins" aria-label={`agent types using ${plugin.name}`}>
        {data
          .filter((type) => !isConversationType(type.name))
          .map((type) => (
            <li key={type.name} className="th-agent-plugin">
              <label>
                <input
                  type="checkbox"
                  checked={using.includes(type.name)}
                  disabled={busy}
                  onChange={(event) => void toggle(type.name, event.target.checked)}
                />
                <span>{type.name}</span>
              </label>
            </li>
          ))}
      </ul>
      <p className="th-cap">a ticked type gets this plugin's skills and slash commands in the next agent that starts. the plugin must be enabled.</p>
    </details>
  );
}
