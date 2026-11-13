export interface KnownMarketplace {
  readonly name: string;
  readonly source: string;
  readonly about: string;
}

export const KNOWN_MARKETPLACES: readonly KnownMarketplace[] = [
  {
    name: "claude-plugins-official",
    source: "anthropics/claude-plugins-official",
    about: "Anthropic's marketplace of Claude Code plugins, with skills like modern-web-guidance",
  },
];

export function missingKnown(have: readonly { readonly name: string }[]): readonly KnownMarketplace[] {
  return KNOWN_MARKETPLACES.filter((known) => !have.some((m) => m.name.toLowerCase() === known.name.toLowerCase()));
}
