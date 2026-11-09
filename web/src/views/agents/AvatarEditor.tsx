import { useState } from "react";
import { saveAgentType } from "../../api/library";
import type { AgentType } from "../../api/types";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { AVATAR_PARTS, cyclePart, type AvatarPart } from "./avatar";
import { AvatarFace } from "./AgentAvatar";
import {
  formatAvatarSpec,
  parseAvatarSpec,
  resolveAvatar,
  withAvatarSection,
  type AvatarSpec,
  type ResolvedAvatar,
} from "./avatarSpec";

const SPEC_HINT = '[avatar]\ncolor = "info"';

const PARTS =Object.keys(AVATAR_PARTS) as readonly AvatarPart[];

function randomSpec(): AvatarSpec {
  const pick = (part: AvatarPart) => AVATAR_PARTS[part][Math.floor(Math.random() * AVATAR_PARTS[part].length)];
  return Object.fromEntries(PARTS.map((part) => [part, pick(part)])) as AvatarSpec;
}

export interface AvatarEditorProps {
  readonly name: string;
  readonly source: string;
  readonly saved: AvatarSpec | null;
  readonly onSaved: (type: AgentType) => void;
}

export function AvatarEditor({ name, source, saved, onSaved }: AvatarEditorProps) {
  const initial = saved === null ? "" : formatAvatarSpec(saved);
  const [text, setText] = useState(initial);
  const [shown, setShown] = useState<ResolvedAvatar>(() => resolveAvatar(name, saved));
  const [specOpen, setSpecOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const parsed = parseAvatarSpec(text);
  const spec = parsed.ok ? parsed.spec : null;
  const current = spec === null ? shown : resolveAvatar(name, spec);

  const edit = (next: string) => {
    setText(next);
    setRefused(null);
    const result = parseAvatarSpec(next);
    if (result.ok) setShown(resolveAvatar(name, result.spec));
  };

  const pickPart = (part: AvatarPart, step: 1 | -1) => {
    if (spec === null) return;
    const next = cyclePart(current.avatar, part, step)[part];
    edit(formatAvatarSpec({ ...spec, [part]: next }));
  };

  const save = () => {
    setBusy(true);
    setRefused(null);
    saveAgentType(name, withAvatarSection(source, text)).then(
      (type) => {
        setBusy(false);
        onSaved(type);
      },
      (error: unknown) => {
        setBusy(false);
        setRefused(error instanceof Error ? error.message : String(error));
      },
    );
  };

  return (
    <div className="th-avatar-editor">
      <AvatarFace avatar={current.avatar} grid={current.grid} scale={6} label={`${name} avatar preview`} />
      <div className="th-avatar-parts">
        {PARTS.map((part) => (
          <div key={part} role="group" aria-label={part} className="th-avatar-part">
            <span className="th-cap">{part}</span>
            <Button size="sm" variant="ghost" aria-label={`previous ${part}`} disabled={spec === null} onClick={() => pickPart(part, -1)}>
              ‹
            </Button>
            <span className="th-avatar-part-value">{current.avatar[part]}</span>
            <Button size="sm" variant="ghost" aria-label={`next ${part}`} disabled={spec === null} onClick={() => pickPart(part, 1)}>
              ›
            </Button>
          </div>
        ))}
        {current.grid === null ? null : (
          <p className="th-cap">grid is set: head, eyes, mouth and accessory are ignored</p>
        )}
        <div className="th-avatar-actions">
          <Button size="sm" variant="ghost" disabled={spec === null} onClick={() => edit(formatAvatarSpec(randomSpec()))}>
            shuffle
          </Button>
          <Button size="sm" variant="ghost" disabled={text.trim() === ""} onClick={() => edit("")}>
            reset
          </Button>
          <Button size="sm" variant="primary" disabled={spec === null || text === initial} busy={busy} onClick={save}>
            save avatar
          </Button>
          <Button size="sm" variant="ghost" aria-expanded={specOpen} onClick={() => setSpecOpen((open) => !open)}>
            edit spec
          </Button>
        </div>
        {specOpen ? (
          <textarea
            className="th-input th-avatar-spec"
            aria-label="avatar spec"
            aria-invalid={parsed.ok ? undefined : true}
            placeholder={SPEC_HINT}
            spellCheck={false}
            rows={5}
            value={text}
            onChange={(event) => edit(event.target.value)}
          />
        ) : null}
        {parsed.ok ? null : (
          <ul aria-label="avatar spec errors" className="th-avatar-errors">
            {parsed.errors.map((message) => (
              <li key={message}>{message}</li>
            ))}
          </ul>
        )}
        {refused === null ? null : <Banner level="error">{refused}</Banner>}
      </div>
    </div>
  );
}
