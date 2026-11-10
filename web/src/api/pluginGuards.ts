import { bool, fail, list, listOf, nullable, oneOf, record, str, strings, type Check, type Guard } from "./guardKit";
import type {
  AvailablePlugin,
  Marketplace,
  MarketplaceEntry,
  MarketplaceSource,
  Plugin,
  PluginList,
  PluginPreview,
  PluginUpdate,
  PluginUpdateOutcome,
} from "./types";

const outcome = oneOf<PluginUpdateOutcome>(["unchanged", "updated", "pinned", "needsApproval"]);

const parseSource: Check<MarketplaceSource> = (value, what) => {
  const r = record(value, what);
  switch (r.source) {
    case "github":
      return { source: "github", repo: str(r.repo, `${what}.repo`) };
    case "url":
      return { source: "url", url: str(r.url, `${what}.url`) };
    case "directory":
      return { source: "directory", path: str(r.path, `${what}.path`) };
    default:
      return fail(`${what}.source`);
  }
};

const parseEntry: Check<MarketplaceEntry> = (value, what) => {
  const r = record(value, what);
  return {
    name: str(r.name, `${what}.name`),
    description: str(r.description, `${what}.description`),
    version: nullable(r.version, `${what}.version`, str),
    category: nullable(r.category, `${what}.category`, str),
  };
};

export const parseMarketplace: Guard<Marketplace> = (value) => {
  const r = record(value, "marketplace");
  return {
    name: str(r.name, "marketplace.name"),
    source: parseSource(r.source, "marketplace.source"),
    account: nullable(r.account, "marketplace.account", str),
    enabled: bool(r.enabled, "marketplace.enabled"),
    official: bool(r.official, "marketplace.official"),
    headSha: nullable(r.headSha, "marketplace.headSha", str),
    fetchedAt: nullable(r.fetchedAt, "marketplace.fetchedAt", str),
    error: nullable(r.error, "marketplace.error", str),
    plugins: list(r.plugins, "marketplace.plugins", parseEntry),
  };
};

export const parseMarketplaces = listOf(parseMarketplace, "marketplaces");

export const parsePlugin: Guard<Plugin> = (value) => {
  const r = record(value, "plugin");
  return {
    id: str(r.id, "plugin.id"),
    marketplace: str(r.marketplace, "plugin.marketplace"),
    name: str(r.name, "plugin.name"),
    version: nullable(r.version, "plugin.version", str),
    sha: str(r.sha, "plugin.sha"),
    enabled: bool(r.enabled, "plugin.enabled"),
    pinned: bool(r.pinned, "plugin.pinned"),
    permissions: strings(r.permissions, "plugin.permissions"),
    permissionsHash: str(r.permissionsHash, "plugin.permissionsHash"),
    installedAt: str(r.installedAt, "plugin.installedAt"),
    updatedAt: str(r.updatedAt, "plugin.updatedAt"),
    error: nullable(r.error, "plugin.error", str),
  };
};

const parseAvailable: Check<AvailablePlugin> = (value, what) => {
  const r = record(value, what);
  return {
    id: str(r.id, `${what}.id`),
    marketplace: str(r.marketplace, `${what}.marketplace`),
    name: str(r.name, `${what}.name`),
    description: str(r.description, `${what}.description`),
    version: nullable(r.version, `${what}.version`, str),
    category: nullable(r.category, `${what}.category`, str),
    installed: bool(r.installed, `${what}.installed`),
  };
};

export const parsePluginList: Guard<PluginList> = (value) => {
  const r = record(value, "plugins");
  return {
    available: list(r.available, "plugins.available", parseAvailable),
    installed: list(r.installed, "plugins.installed", (item) => parsePlugin(item)),
  };
};

export const parsePreview: Guard<PluginPreview> = (value) => {
  const r = record(value, "preview");
  return {
    plugin: str(r.plugin, "preview.plugin"),
    marketplace: str(r.marketplace, "preview.marketplace"),
    name: str(r.name, "preview.name"),
    version: nullable(r.version, "preview.version", str),
    sha: str(r.sha, "preview.sha"),
    permissions: strings(r.permissions, "preview.permissions"),
    permissionsHash: str(r.permissionsHash, "preview.permissionsHash"),
  };
};

export const parsePluginUpdate: Guard<PluginUpdate> = (value) => {
  const r = record(value, "update");
  return {
    outcome: outcome(r.outcome, "update.outcome"),
    sha: nullable(r.sha, "update.sha", str),
    added: strings(r.added ?? [], "update.added"),
    permissionsHash: nullable(r.permissionsHash, "update.permissionsHash", str),
    plugin: parsePlugin(r.plugin),
  };
};
