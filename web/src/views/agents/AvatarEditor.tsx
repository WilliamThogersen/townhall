import { useState } from "react";
import { Button } from "../../ui/Button";
import { AVATAR_PARTS, cyclePart, defaultAvatar, type Avatar, type AvatarPart } from "./avatar";
import { AvatarFace } from "./AgentAvatar";
import { clearAvatar, saveAvatar, useAvatar } from "./avatarPrefs";

const PARTS = Object.keys(AVATAR_PARTS) as readonly AvatarPart[];

const sameAvatar = (a: Avatar, b: Avatar) => PARTS.every((part) => a[part] === b[part]);

function randomAvatar(): Avatar {
  const pick = (part: AvatarPart) => AVATAR_PARTS[part][Math.floor(Math.random() * AVATAR_PARTS[part].length)];
  return Object.fromEntries(PARTS.map((part) => [part, pick(part)])) as unknown as Avatar;
}

export function AvatarEditor({ name }: { readonly name: string }) {
  const avatar = useAvatar(name);
  const [notSaved, setNotSaved] = useState(false);
  const customised = !sameAvatar(avatar, defaultAvatar(name));
  const commit = (saved: boolean) => setNotSaved(!saved);
  return (
    <div className="th-avatar-editor">
      <AvatarFace avatar={avatar} scale={8} label={`${name} avatar preview`} />
      <div className="th-avatar-parts">
        {PARTS.map((part) => (
          <div key={part} role="group" aria-label={part} className="th-avatar-part">
            <span className="th-cap">{part}</span>
            <Button size="sm" variant="ghost" aria-label={`previous ${part}`} onClick={() => commit(saveAvatar(name, cyclePart(avatar, part, -1)))}>
              ‹
            </Button>
            <span className="th-avatar-part-value">{avatar[part]}</span>
            <Button size="sm" variant="ghost" aria-label={`next ${part}`} onClick={() => commit(saveAvatar(name, cyclePart(avatar, part, 1)))}>
              ›
            </Button>
          </div>
        ))}
        <div className="th-avatar-actions">
          <Button size="sm" variant="ghost" onClick={() => commit(saveAvatar(name, randomAvatar()))}>
            shuffle
          </Button>
          <Button size="sm" variant="ghost" disabled={!customised} onClick={() => commit(clearAvatar(name))}>
            reset
          </Button>
        </div>
        {notSaved ? (
          <p role="status" className="th-cap">
            not saved: browser storage refused the write
          </p>
        ) : null}
      </div>
    </div>
  );
}
