import { bool, fail, list, listOf, nullable, num, oneOf, record, str, strings, type Check, type Guard } from "./guardKit";
import type {
  ActivePluginTheme,
  AvailablePlugin,
  HostToken,
  Marketplace,
  MarketplaceEntry,
  MarketplaceSource,
  Plugin,
  PluginContributions,
  PluginList,
  PluginPreview,
  PluginSettingField,
  PluginSettingsGroup,
  PluginStarter,
  PluginThemeChoice,
  PluginUiEntry,
  PluginUpdate,
  PluginUpdateOutcome,
  SettingFieldType,
  SettingFieldValue,
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

const FIELD_TYPES = oneOf<SettingFieldType>(["boolean", "string", "integer", "enum"]);

const fieldValue: Check<SettingFieldValue> = (value, what) =>
  typeof value === "boolean" || typeof value === "string" || (typeof value === "number" && Number.isFinite(value))
    ? value
    : fail(what);

const parseField: Check<PluginSettingField> = (value, what) => {
  const r = record(value, what);
  return {
    key: str(r.key, `${what}.key`),
    label: str(r.label, `${what}.label`),
    description: nullable(r.description, `${what}.description`, str),
    type: FIELD_TYPES(r.type, `${what}.type`),
    default: fieldValue(r.default, `${what}.default`),
    min: nullable(r.min, `${what}.min`, num),
    max: nullable(r.max, `${what}.max`, num),
    choices: nullable(r.choices, `${what}.choices`, strings),
    secret: bool(r.secret, `${what}.secret`),
    value: nullable(r.value, `${what}.value`, fieldValue),
    set: bool(r.set, `${what}.set`),
  };
};

const parseGroup: Check<PluginSettingsGroup> = (value, what) => {
  const r = record(value, what);
  return {
    plugin: str(r.plugin, `${what}.plugin`),
    name: str(r.name, `${what}.name`),
    fields: list(r.fields, `${what}.fields`, parseField),
  };
};

const parseStarter: Check<PluginStarter> = (value, what) => {
  const r = record(value, what);
  return {
    plugin: str(r.plugin, `${what}.plugin`),
    title: str(r.title, `${what}.title`),
    text: str(r.text, `${what}.text`),
  };
};

const parseThemeChoice: Check<PluginThemeChoice> = (value, what) => {
  const r = record(value, what);
  return { plugin: str(r.plugin, `${what}.plugin`), name: str(r.name, `${what}.name`) };
};

const parseActiveTheme: Check<ActivePluginTheme> = (value, what) => {
  const r = record(value, what);
  return { plugin: str(r.plugin, `${what}.plugin`), css: str(r.css, `${what}.css`) };
};

export const parseContributions: Guard<PluginContributions> = (value) => {
  const r = record(value, "contributions");
  return {
    starters: list(r.starters, "contributions.starters", parseStarter),
    themes: list(r.themes, "contributions.themes", parseThemeChoice),
    theme: nullable(r.theme, "contributions.theme", parseActiveTheme),
    settings: list(r.settings, "contributions.settings", parseGroup),
  };
};

const parseUiEntry: Check<PluginUiEntry> = (value, what) => {
  const r = record(value, what);
  return {
    plugin: str(r.plugin, `${what}.plugin`),
    slot: str(r.slot, `${what}.slot`),
    title: str(r.title, `${what}.title`),
    entry: str(r.entry, `${what}.entry`),
    height: nullable(r.height, `${what}.height`, num),
  };
};

export const parseUiEntries: Guard<readonly PluginUiEntry[]> = (value) => list(value, "ui", parseUiEntry);

export const parseHostToken: Guard<HostToken> = (value) => {
  const r = record(value, "hostToken");
  return {
    plugin: str(r.plugin, "hostToken.plugin"),
    version: nullable(r.version, "hostToken.version", str),
    token: str(r.token, "hostToken.token"),
    scopes: strings(r.scopes, "hostToken.scopes"),
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
