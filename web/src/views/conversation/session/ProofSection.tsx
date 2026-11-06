import { useState } from "react";
import { artifactUrl, readArtifactText } from "../../../api/artifacts";
import type { Artifact } from "../../../api/types";
import { formatSize, Lightbox, type LightboxImage } from "../../../attachments";
import { formatClock } from "../../../attachments/format";
import { useArtifacts } from "../../../data";
import { useLoad } from "../../../data/useLoad";
import { isTextProof, proofGroups } from "./proofModel";

interface Opened {
  readonly index: number;
  readonly opener: HTMLElement;
}

function lightboxOf(task: string, shots: readonly Artifact[]): readonly LightboxImage[] {
  return shots.map((shot) => ({
    src: artifactUrl(task, shot.id),
    name: shot.name,
    alt: shot.caption ?? shot.name,
    meta: [formatSize(shot.size), `from ${shot.authorRole}`, formatClock(shot.createdAt)].filter(Boolean).join(" · "),
  }));
}

function Caption({ artifact }: { readonly artifact: Artifact }) {
  if (artifact.caption === null) return null;
  return <p className="th-proof-caption">{artifact.caption}</p>;
}

interface TextProofProps {
  readonly task: string;
  readonly artifact: Artifact;
}

function TextProof({ task, artifact }: TextProofProps) {
  const [open, setOpen] = useState(false);
  const { data, loading, error } = useLoad(
    open ? JSON.stringify(["artifact-text", task, artifact.id]) : null,
    (signal) => readArtifactText(task, artifact.id, signal),
    0,
  );
  return (
    <>
      <div className="th-proof-head">
        <span className="th-proof-name" title={artifact.name}>
          {artifact.name}
        </span>
        <span className="th-proof-meta">{`${artifact.kind} · ${formatSize(artifact.size)}`}</span>
        <button
          type="button"
          className="th-btn th-btn--ghost th-btn--sm"
          aria-expanded={open}
          aria-label={`${open ? "hide" : "show"} ${artifact.name}`}
          onClick={() => setOpen((was) => !was)}
        >
          {open ? "hide" : "show"}
        </button>
      </div>
      <Caption artifact={artifact} />
      {open && error !== null && <p role="alert" className="th-proof-error">{error}</p>}
      {open && error === null && loading && <p className="th-proof-meta">loading</p>}
      {open && data !== null && (
        <pre className="th-proof-text" aria-label={`${artifact.name} contents`}>
          {data}
        </pre>
      )}
    </>
  );
}

interface ShotProps {
  readonly task: string;
  readonly artifact: Artifact;
  readonly onOpen: (opener: HTMLElement) => void;
}

function ShotProof({ task, artifact, onOpen }: ShotProps) {
  const alt = artifact.caption ?? artifact.name;
  return (
    <>
      <button
        type="button"
        className="th-thumb th-proof-thumb"
        aria-label={`open ${artifact.name}`}
        onClick={(event) => onOpen(event.currentTarget)}
      >
        <img src={artifactUrl(task, artifact.id)} alt={alt} loading="lazy" decoding="async" />
      </button>
      <div className="th-proof-head">
        <span className="th-proof-name" title={artifact.name}>
          {artifact.name}
        </span>
        <span className="th-proof-meta">{formatSize(artifact.size)}</span>
      </div>
      <Caption artifact={artifact} />
    </>
  );
}

export function ProofSection({ task }: { readonly task: string }) {
  const { artifacts, loading, error } = useArtifacts(task);
  const [opened, setOpened] = useState<Opened | null>(null);
  const groups = proofGroups(artifacts);
  const shots = groups.flatMap((group) => group.items.filter((item) => !isTextProof(item)));
  return (
    <>
      <h4 className="th-section">proof {artifacts.length}</h4>
      {error !== null && artifacts.length === 0 && (
        <div className="th-session-line">
          <span>cannot load proof: {error}</span>
        </div>
      )}
      {error === null && artifacts.length === 0 && (
        <div className="th-session-line">
          <span>{loading ? "loading proof" : "no proof yet"}</span>
        </div>
      )}
      {groups.map((group) => (
        <section key={group.role} className="th-proof-group" data-role={group.role} aria-label={`${group.label} proof`}>
          <h5 className="th-proof-role">
            {group.label} <span>{group.items.length}</span>
          </h5>
          <ul className="th-proof-list">
            {group.items.map((artifact) => (
              <li key={artifact.id} className="th-proof-item" data-kind={artifact.kind}>
                {isTextProof(artifact) ? (
                  <TextProof task={task} artifact={artifact} />
                ) : (
                  <ShotProof
                    task={task}
                    artifact={artifact}
                    onOpen={(opener) => setOpened({ index: shots.indexOf(artifact), opener })}
                  />
                )}
              </li>
            ))}
          </ul>
        </section>
      ))}
      {opened && (
        <Lightbox
          images={lightboxOf(task, shots)}
          startIndex={opened.index}
          returnFocusTo={opened.opener}
          onClose={() => setOpened(null)}
        />
      )}
    </>
  );
}
