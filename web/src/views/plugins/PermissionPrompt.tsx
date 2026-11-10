import { useEffect } from "react";
import { createPortal } from "react-dom";
import type { PluginPreview } from "../../api/types";
import { groupPermissions } from "../../plugins/permissions";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { Loading } from "../../ui/Loading";
import "./plugins.css";

export function PermissionList({ permissions }: { readonly permissions: readonly string[] }) {
  const groups = groupPermissions(permissions);
  if (groups.length === 0) return <p className="th-cap">asks for no permissions</p>;
  return (
    <div className="th-perms">
      {groups.map((group) => (
        <section key={group.id} role="group" aria-label={group.title} className="th-perm-group">
          <h3 className="th-perm-title">{group.title}</h3>
          <ul className="th-perm-items">
            {group.items.map((item) => (
              <li key={item.raw} title={item.raw}>
                <span>{item.text}</span>
                {item.flag === null ? null : <span className="th-tag th-tag--waiting">{item.flag}</span>}
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}

export interface PermissionPromptProps {
  readonly id: string;
  readonly official: boolean;
  readonly preview: PluginPreview | null;
  readonly error: string | null;
  readonly busy: boolean;
  readonly onInstall: (enable: boolean) => void;
  readonly onCancel: () => void;
}

function useEscape(onCancel: () => void) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onCancel]);
}

export function PermissionPrompt({ id, official, preview, error, busy, onInstall, onCancel }: PermissionPromptProps) {
  useEscape(onCancel);
  const name = preview?.name ?? id.split("@")[0] ?? id;
  const ready = preview !== null && !busy;
  return createPortal(
    <div className="th-plugin-modal">
      <div className="th-plugin-modal-box" role="dialog" aria-modal="true" aria-label={`install ${name}`}>
        <header className="th-plugin-prompt-head">
          <h2>{`install ${name}`}</h2>
          {official ? <span className="th-tag th-tag--ok">official</span> : null}
        </header>
        <p className="th-cap th-plugin-prompt-meta">
          <span>{id}</span>
          {preview?.version ? <span>{preview.version}</span> : null}
          {preview ? <span title={preview.sha}>{preview.sha.slice(0, 7)}</span> : null}
        </p>
        {error === null ? null : <Banner level="error">{error}</Banner>}
        {preview === null && error === null ? <Loading verb="loading permissions" /> : null}
        {preview === null ? null : (
          <>
            <p className="th-plugin-prompt-lead">installing lets this plugin do the following:</p>
            <PermissionList permissions={preview.permissions} />
          </>
        )}
        <div className="th-plugin-prompt-actions">
          <Button variant="ghost" onClick={onCancel}>
            cancel
          </Button>
          <Button disabled={!ready} onClick={() => onInstall(false)}>
            install
          </Button>
          <Button variant="primary" disabled={!ready} busy={busy} busyLabel="installing" onClick={() => onInstall(true)}>
            install and enable
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
