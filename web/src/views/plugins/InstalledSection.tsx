import { useState } from "react";
import type { Plugin, PluginUpdate } from "../../api/types";
import type { PluginsState } from "../../data";
import { pendingUpdateSha, pluginStates, type PluginStateName } from "../../plugins/pluginState";
import { Button } from "../../ui/Button";
import { PermissionList } from "./PermissionPrompt";
import type { Run } from "./useInstallFlow";
import "./plugins.css";

interface Review {
  readonly added: readonly string[];
  readonly hash: string;
}

const STATE_TAG: Readonly<Record<PluginStateName, string>> = {
  installed: "th-tag",
  enabled: "th-tag th-tag--ok",
  pinned: "th-tag",
  "needs approval": "th-tag th-tag--waiting",
};

function outcomeNote(update: PluginUpdate): string | null {
  switch (update.outcome) {
    case "updated":
      return `updated to ${update.sha ?? update.plugin.sha}`;
    case "unchanged":
      return "up to date";
    case "pinned":
      return "pinned, not updated";
    case "needsApproval":
      return null;
  }
}

interface RowProps {
  readonly plugin: Plugin;
  readonly plugins: PluginsState;
  readonly run: Run;
}

function UpdateReview({ plugin, review, busy, onApprove, onDismiss }: {
  readonly plugin: Plugin;
  readonly review: Review;
  readonly busy: boolean;
  readonly onApprove: () => void;
  readonly onDismiss: () => void;
}) {
  return (
    <div className="th-plugin-extra">
      <p className="th-plugin-note">this update asks for more than you approved:</p>
      <PermissionList permissions={review.added} />
      <div className="th-plugin-confirm">
        <Button variant="ghost" size="sm" disabled={busy} onClick={onDismiss}>
          dismiss
        </Button>
        <Button variant="primary" size="sm" disabled={busy} aria-label={`approve update ${plugin.name}`} onClick={onApprove}>
          approve update
        </Button>
      </div>
    </div>
  );
}

function UninstallConfirm({ plugin, busy, onConfirm, onCancel }: {
  readonly plugin: Plugin;
  readonly busy: boolean;
  readonly onConfirm: (purge: boolean) => void;
  readonly onCancel: () => void;
}) {
  const [purge, setPurge] = useState(false);
  return (
    <div className="th-plugin-confirm">
      <span>{`uninstall ${plugin.name}?`}</span>
      <label>
        <input type="checkbox" checked={purge} onChange={(event) => setPurge(event.target.checked)} /> also delete its data
      </label>
      <Button variant="ghost" size="sm" disabled={busy} onClick={onCancel}>
        keep it
      </Button>
      <Button variant="danger" size="sm" disabled={busy} aria-label="confirm uninstall" onClick={() => onConfirm(purge)}>
        uninstall
      </Button>
    </div>
  );
}

function InstalledRow({ plugin, plugins, run }: RowProps) {
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [review, setReview] = useState<Review | null>(null);
  const [confirming, setConfirming] = useState(false);
  const waiting = pendingUpdateSha(plugin) !== null;

  const go = async <T,>(action: () => Promise<T>): Promise<T | undefined> => {
    setBusy(true);
    try {
      return await run(action);
    } finally {
      setBusy(false);
    }
  };

  const update = async (approve?: string) => {
    const result = await go(() => (approve === undefined ? plugins.update(plugin.id) : plugins.update(plugin.id, approve)));
    if (result === undefined) return;
    const text = outcomeNote(result);
    setNote(text);
    setReview(
      result.outcome === "needsApproval" && result.permissionsHash !== null
        ? { added: result.added, hash: result.permissionsHash }
        : null,
    );
  };

  return (
    <article className="th-plugin-row" aria-label={plugin.id}>
      <div className="th-plugin-name">
        <b>{plugin.name}</b>
        {plugin.version === null ? null : <span className="th-cap">{plugin.version}</span>}
        {pluginStates(plugin).map((state) => (
          <span key={state} className={STATE_TAG[state]}>
            {state}
          </span>
        ))}
      </div>
      <p className="th-plugin-meta">
        <span>{plugin.id}</span>
        <span title={plugin.sha}>{plugin.sha.slice(0, 7)}</span>
      </p>
      {plugin.error === null ? null : <p className="th-plugin-meta th-plugin-error">{plugin.error}</p>}
      {note === null ? null : <p className="th-plugin-meta">{note}</p>}
      {confirming ? (
        <UninstallConfirm
          plugin={plugin}
          busy={busy}
          onCancel={() => setConfirming(false)}
          onConfirm={(purge) => void go(() => plugins.uninstall(plugin.id, purge))}
        />
      ) : (
        <div className="th-plugin-actions">
          <Button
            size="sm"
            disabled={busy}
            aria-label={`${plugin.enabled ? "disable" : "enable"} ${plugin.name}`}
            onClick={() => void go(() => plugins.patch(plugin.id, { enabled: !plugin.enabled }))}
          >
            {plugin.enabled ? "disable" : "enable"}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={busy}
            aria-label={`${plugin.pinned ? "unpin" : "pin"} ${plugin.name}`}
            onClick={() => void go(() => plugins.patch(plugin.id, { pinned: !plugin.pinned }))}
          >
            {plugin.pinned ? "unpin" : "pin"}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={busy}
            aria-label={`${waiting ? "review update" : "update"} ${plugin.name}`}
            onClick={() => void update()}
          >
            {waiting ? "review update" : "update"}
          </Button>
          <Button variant="ghost" size="sm" disabled={busy} aria-label={`uninstall ${plugin.name}`} onClick={() => setConfirming(true)}>
            uninstall
          </Button>
        </div>
      )}
      {review === null ? null : (
        <UpdateReview
          plugin={plugin}
          review={review}
          busy={busy}
          onApprove={() => void update(review.hash)}
          onDismiss={() => setReview(null)}
        />
      )}
      <details className="th-plugin-extra">
        <summary>permissions</summary>
        <PermissionList permissions={plugin.permissions} />
      </details>
    </article>
  );
}

export function InstalledSection({ plugins, run }: { readonly plugins: PluginsState; readonly run: Run }) {
  return (
    <section className="th-plugins-section" aria-label="installed plugins">
      <header className="th-plugins-section-head">
        <h2>installed</h2>
        <span className="th-cap">{plugins.installed.length}</span>
      </header>
      {plugins.installed.length === 0 ? <p className="th-plugin-note th-plugin-row">no plugins installed</p> : null}
      {plugins.installed.map((plugin) => (
        <InstalledRow key={plugin.id} plugin={plugin} plugins={plugins} run={run} />
      ))}
    </section>
  );
}
