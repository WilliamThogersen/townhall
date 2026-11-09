import { useEffect, useState } from "react";
import { toMessage } from "../../api/errors";
import { getDetectedAccounts, type DetectedAccount } from "../../api/forgeAccounts";
import type { ForgeAccount } from "../../settings/settings";
import { Button } from "../../ui/Button";
import "./detected.css";

const same = (a: ForgeAccount, b: DetectedAccount) =>
  a.forge === b.forge && a.host === b.host && a.login.toLowerCase() === b.login.toLowerCase();

const asAccount = ({ forge, login, host }: DetectedAccount): ForgeAccount => ({ forge, login, host, label: login });

interface Props {
  readonly accounts: readonly ForgeAccount[];
  readonly onAdd: (added: readonly ForgeAccount[]) => void;
}

export function DetectedAccounts({ accounts, onAdd }: Props) {
  const [found, setFound] = useState<readonly DetectedAccount[] | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    getDetectedAccounts(controller.signal).then(setFound, (e: unknown) => {
      if (!controller.signal.aborted) setProblem(toMessage(e));
    });
    return () => controller.abort();
  }, []);

  if (problem !== null) return <p className="th-cap">could not look for logged-in accounts: {problem}</p>;
  if (found === null) return null;
  const pending = found.filter((d) => !accounts.some((a) => same(a, d)));
  if (found.length === 0) {
    return (
      <p className="th-cap">
        no logged-in accounts found: <code>gh auth status</code> and <code>glab auth status</code> listed none, or
        neither program was found on this machine
      </p>
    );
  }
  if (pending.length === 0) return null;
  const first = accounts.length === 0;
  const count = `${pending.length} account${pending.length === 1 ? "" : "s"}`;
  return (
    <div
      className="th-detected"
      role="group"
      aria-label="Detected accounts"
      data-prominent={first ? "true" : undefined}
    >
      <p className={first ? undefined : "th-cap"}>
        {first ? `found ${count} logged in with gh and glab: add them to use them` : "also logged in on this machine:"}
      </p>
      <div className="th-detected-chips">
        {pending.map((d) => (
          <Button
            key={`${d.forge}/${d.host}/${d.login}`}
            size="sm"
            aria-label={`add ${d.login}@${d.host}`}
            title={`${d.forge} · ${d.login}@${d.host}`}
            onClick={() => onAdd([asAccount(d)])}
          >
            {`+ ${d.login}@${d.host}`}
          </Button>
        ))}
        {pending.length > 1 || first ? (
          <Button
            size="sm"
            variant={first ? "primary" : "default"}
            onClick={() => onAdd(pending.map(asAccount))}
          >
            add all
          </Button>
        ) : null}
      </div>
    </div>
  );
}
