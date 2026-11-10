import type { AvailablePlugin, Marketplace } from "../../api/types";
import { Button } from "../../ui/Button";
import "./plugins.css";

interface Props {
  readonly marketplaces: readonly Marketplace[];
  readonly available: readonly AvailablePlugin[];
  readonly onInstall: (id: string) => void;
}

function AvailableRow({ plugin, onInstall }: { readonly plugin: AvailablePlugin; readonly onInstall: Props["onInstall"] }) {
  return (
    <article className="th-plugin-row" aria-label={plugin.id}>
      <div className="th-plugin-name">
        <b>{plugin.name}</b>
        {plugin.version === null ? null : <span className="th-cap">{plugin.version}</span>}
        {plugin.category === null ? null : <span className="th-tag">{plugin.category}</span>}
        {plugin.installed ? <span className="th-tag th-tag--ok">installed</span> : null}
      </div>
      <p className="th-plugin-desc">{plugin.description}</p>
      {plugin.installed ? null : (
        <div className="th-plugin-actions">
          <Button size="sm" aria-label={`install ${plugin.name}`} onClick={() => onInstall(plugin.id)}>
            install
          </Button>
        </div>
      )}
    </article>
  );
}

export function AvailableSection({ marketplaces, available, onInstall }: Props) {
  const groups = marketplaces
    .map((market) => ({ market, items: available.filter((p) => p.marketplace.toLowerCase() === market.name.toLowerCase()) }))
    .filter((group) => group.items.length > 0);
  return (
    <section className="th-plugins-section" aria-label="available plugins">
      <header className="th-plugins-section-head">
        <h2>available</h2>
        <span className="th-cap">{available.length}</span>
      </header>
      {groups.length === 0 ? <p className="th-plugin-note th-plugin-row">no plugins listed by your marketplaces</p> : null}
      {groups.map(({ market, items }) => (
        <div key={market.name}>
          <h3 className="th-perm-title th-plugin-row">{market.name}</h3>
          {items.map((plugin) => (
            <AvailableRow key={plugin.id} plugin={plugin} onInstall={onInstall} />
          ))}
        </div>
      ))}
    </section>
  );
}
