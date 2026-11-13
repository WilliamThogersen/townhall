import { useState, type FormEvent } from "react";
import type { Marketplace, MarketplaceSource } from "../../api/types";
import type { PluginsState } from "../../data";
import { missingKnown, type KnownMarketplace } from "../../plugins/knownMarketplaces";
import { Button } from "../../ui/Button";
import type { Run } from "./useInstallFlow";
import "./plugins.css";

export function sourceText(source: MarketplaceSource): string {
  switch (source.source) {
    case "github":
      return `https://github.com/${source.repo}`;
    case "url":
      return source.url;
    case "directory":
      return source.path;
  }
}

const count = (n: number): string => (n === 1 ? "1 plugin" : `${n} plugins`);

interface RowProps {
  readonly market: Marketplace;
  readonly plugins: PluginsState;
  readonly run: Run;
}

function MarketplaceRow({ market, plugins, run }: RowProps) {
  const [busy, setBusy] = useState(false);
  const go = async (action: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await run(action);
    } finally {
      setBusy(false);
    }
  };
  return (
    <article className="th-plugin-row" aria-label={market.name}>
      <div className="th-plugin-name">
        <b>{market.name}</b>
        {market.official ? <span className="th-tag th-tag--ok">official</span> : null}
        {market.enabled ? null : <span className="th-tag">hidden</span>}
      </div>
      <p className="th-plugin-meta">
        <span>{sourceText(market.source)}</span>
        <span>{count(market.plugins.length)}</span>
        {market.headSha === null ? null : <span title={market.headSha}>{market.headSha.slice(0, 7)}</span>}
        {market.account === null ? null : <span>{`account ${market.account}`}</span>}
      </p>
      {market.error === null ? null : <p className="th-plugin-meta th-plugin-error">{market.error}</p>}
      <div className="th-plugin-actions">
        <Button
          variant="ghost"
          size="sm"
          busy={busy}
          aria-label={`refresh ${market.name}`}
          onClick={() => void go(() => plugins.refreshMarketplace(market.name))}
        >
          refresh
        </Button>
        {market.official ? null : (
          <Button
            variant="ghost"
            size="sm"
            disabled={busy}
            aria-label={`remove ${market.name}`}
            onClick={() => void go(() => plugins.removeMarketplace(market.name))}
          >
            remove
          </Button>
        )}
      </div>
    </article>
  );
}

function AddMarketplace({ plugins, run }: { readonly plugins: PluginsState; readonly run: Run }) {
  const [source, setSource] = useState("");
  const [busy, setBusy] = useState(false);
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    try {
      const added = await run(() => plugins.addMarketplace({ source: source.trim() }));
      if (added !== undefined) setSource("");
    } finally {
      setBusy(false);
    }
  };
  return (
    <form className="th-plugin-add" onSubmit={(event) => void submit(event)}>
      <input
        type="text"
        className="th-input"
        aria-label="marketplace link, owner/repo or folder"
        placeholder="link, owner/repo or folder"
        autoComplete="off"
        autoCapitalize="off"
        spellCheck={false}
        value={source}
        onChange={(event) => setSource(event.target.value)}
      />
      <Button type="submit" variant="primary" size="sm" busy={busy} busyLabel="adding" disabled={source.trim() === ""}>
        add marketplace
      </Button>
    </form>
  );
}

function KnownMarketplaces({ plugins, run }: { readonly plugins: PluginsState; readonly run: Run }) {
  const [busy, setBusy] = useState<string | null>(null);
  const add = async (known: KnownMarketplace) => {
    setBusy(known.name);
    try {
      await run(() => plugins.addMarketplace({ source: known.source }));
    } finally {
      setBusy(null);
    }
  };
  return (
    <>
      {missingKnown(plugins.marketplaces).map((known) => (
        <article key={known.name} className="th-plugin-row" aria-label={`known marketplace ${known.name}`}>
          <div className="th-plugin-name">
            <b>{known.name}</b>
          </div>
          <p className="th-plugin-meta">
            <span>{known.about}</span>
          </p>
          <div className="th-plugin-actions">
            <Button
              size="sm"
              busy={busy === known.name}
              busyLabel="adding"
              disabled={busy !== null}
              aria-label={`add ${known.name}`}
              onClick={() => void add(known)}
            >
              add
            </Button>
          </div>
        </article>
      ))}
    </>
  );
}

export function MarketplaceSection({ plugins, run }: { readonly plugins: PluginsState; readonly run: Run }) {
  return (
    <section className="th-plugins-section" aria-label="marketplaces">
      <header className="th-plugins-section-head">
        <h2>marketplaces</h2>
        <span className="th-cap">{plugins.marketplaces.length}</span>
      </header>
      <AddMarketplace plugins={plugins} run={run} />
      <KnownMarketplaces plugins={plugins} run={run} />
      {plugins.marketplaces.map((market) => (
        <MarketplaceRow key={market.name} market={market} plugins={plugins} run={run} />
      ))}
    </section>
  );
}
