import { Link, PATHS, useRoute } from "../router";
import { useUiEntries, type ListUi } from "./useUiEntries";

const NAV_GLYPH = "*";

export function PluginNavItems({ listUi }: { readonly listUi?: ListUi }) {
  const route = useRoute();
  const entries = useUiEntries("sidebar.nav", listUi);
  return (
    <>
      {entries.map((entry) => {
        const to = PATHS.pluginPage(entry.plugin, entry.entry);
        const current = route.name === "plugin-page" && route.params.plugin === entry.plugin && route.params.page === entry.entry;
        return (
          <Link key={`${entry.plugin}/${entry.entry}`} className="th-nav-item" to={to} aria-current={current ? "page" : undefined}>
            <span className="th-nav-icon" aria-hidden="true">{NAV_GLYPH}</span>
            <span>{entry.title}</span>
          </Link>
        );
      })}
    </>
  );
}
