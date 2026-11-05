import type { Artifact, AuthorRole } from "../../../api/types";

export interface ProofGroup {
  readonly role: AuthorRole;
  readonly label: string;
  readonly items: readonly Artifact[];
}

const ROLE_ORDER: readonly AuthorRole[] = ["coder", "tester", "reviewer", "other"];

export function proofGroups(artifacts: readonly Artifact[]): readonly ProofGroup[] {
  return ROLE_ORDER.flatMap((role) => {
    const items = artifacts.filter((artifact) => artifact.authorRole === role);
    return items.length > 0 ? [{ role, label: role, items }] : [];
  });
}

export function isTextProof(artifact: Artifact): boolean {
  return artifact.kind !== "screenshot";
}
