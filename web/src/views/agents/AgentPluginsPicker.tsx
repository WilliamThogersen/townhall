import { useState } from "react";
import { toMessage } from "../../api/errors";
import { saveAgentType } from "../../api/library";
import type { AgentType, Plugin } from "../../api/types";
import { useInstalledPlugins } from "../../data";
import { Link, PATHS } from "../../router";
import { Banner } from "../../ui/Banner";
import { isChosen, pluginsOf, shipped, shippedText, withPlugin } from "./agentPlugins";
import { isConversationType } from "./agentTypes";
import "../plugins/plugins.css";

const NONE: readonly Plugin[] = [];

interface RowProps {
  readonly id: string;
  readonly name: string;
  readonly plugin: Plugin | null;
  readonly checked: boolean;
  readonly disabled: boolean;
  readonly onToggle: (on: boolean) => void;
}

function PluginRow({ id, name, plugin, checked, disabled, onToggle }: RowProps) {
  const adds = plugin === null ? "" : shippedText(shipped(plugin));
  return (
    <li className="th-agent-plugin" title={id}>
      <label>
        <input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onToggle(event.target.checked)} />
        <span>{name}</span>
      </label>
      {plugin === null ? <span className="th-tag th-tag--waiting">not installed</span> : <span className="th-cap">{plugin.marketplace}</span>}
      {plugin === null || plugin.enabled ? null : <span className="th-tag">disabled</span>}
      {adds === "" ? null : <span className="th-cap">{adds}</span>}
    </li>
  );
}

export function AgentPluginsPicker({ type, onSaved }: { readonly type: AgentType; readonly onSaved: () => void }) {
  const { data, error } = useInstalledPlugins();
  const installed = data ?? NONE;
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const chosen = pluginsOf(type);
  const fromPlugin = type.origin === "plugin";

  if (isConversationType(type.name)) {
    return <p className="th-cap">townhall's own session never loads plugin skills: it only plans and reads.</p>;
  }

  const toggle = (id: string, on: boolean) => {
    setBusy(true);
    setRefused(null);
    saveAgentType(type.name, withPlugin(type.source, chosen, id, on)).then(
      () => {
        setBusy(false);
        onSaved();
      },
      (error: unknown) => {
        setBusy(false);
        setRefused(toMessage(error));
      },
    );
  };

  if (data === null) {
    return <p className="th-cap">{error === null ? "loading plugins" : `plugins not loaded: ${error}`}</p>;
  }
  const gone = chosen.filter((id) => !installed.some((plugin) => isChosen([id], plugin.id)));
  if (installed.length === 0 && gone.length === 0) {
    return (
      <p className="th-cap">
        no plugins installed: add one on the <Link to={PATHS.plugins}>plugins</Link> page, then choose it here.
      </p>
    );
  }
  return (
    <>
      {fromPlugin ? <p className="th-cap">this type comes from a plugin and is read only: copy it to change what it uses.</p> : null}
      {refused === null ? null : <Banner level="error">{refused}</Banner>}
      <ul className="th-agent-plugins" aria-label="plugins this type uses">
        {installed.map((plugin) => (
          <PluginRow
            key={plugin.id}
            id={plugin.id}
            name={plugin.name}
            plugin={plugin}
            checked={isChosen(chosen, plugin.id)}
            disabled={busy || fromPlugin}
            onToggle={(on) => toggle(plugin.id, on)}
          />
        ))}
        {gone.map((id) => (
          <PluginRow key={id} id={id} name={id} plugin={null} checked disabled={busy || fromPlugin} onToggle={(on) => toggle(id, on)} />
        ))}
      </ul>
      <p className="th-cap">the skills and slash commands of a ticked plugin load in the next agent of this type that starts. a plugin must be enabled.</p>
    </>
  );
}
