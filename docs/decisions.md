# Open design decisions

Made on your behalf so the guide could be concrete. Veto any of them; each says what changes.

| # | Decision | Chosen | Alternative | Cost to change |
|---|----------|--------|-------------|----------------|
| 1 | Brass accent (light) | `#7a5a14` (dark brass, white text on it: 6.4:1) | brighter `#a8741a` fails AA with white text | One token, plus `on-accent` |
| 2 | Brass accent (dark) | `#d2a64a` with near-black text (8.1:1) | a paler gold | One token |
| 3 | Fonts | System stacks only, no web fonts. Display: Iowan Old Style / Palatino / Georgia. UI: system-ui. Mono: SF Mono / Menlo / Consolas | Self-host Source Serif 4, Inter, JetBrains Mono (woff2 in `web/public/fonts`, `font-display: swap`) | Add `@font-face`; no component changes |
| 4 | Density default | Comfortable (36px controls, 336px columns). Compact is a toggle (28px, 280px) | Compact as default given 10-50 agents | One attribute default |
| 5 | Activity page default density | Compact is recommended once more than 12 agents exist | Always follow the global setting | Small |
| 6 | Waiting color | Ochre-orange (`--th-waiting`), distinct from brass and from red | Reuse brass | Brass then means both "clickable" and "needs you" |
| 7 | Dead vs error | Dead is neutral stone with an X box (ended, not necessarily wrong). Error is red with a triangle. Today the app shows dead as red | Dead red as today | Two tokens |
| 8 | Status vocabulary | Five states: alive, working, waiting, dead, error. The backend only reports `alive` today; working, waiting and error need daemon support (see components-agents.md) | Ship alive/dead only first | None to the design |
| 9 | Focus ring color | Ink (text color), 2px, offset 2px, in both themes. Not brass | Brass ring | One token |
| 10 | Theme control | Auto by default (follows OS), manual override stored in localStorage as `th-theme` | Dark only (today) | Small |
| 11 | Nav | Two tabs: Activity (`/`) and Agents (`/agents`). The create form moves into a dialog opened from a header button | Keep create form inline on Agents | Moderate, see components.md |
| 12 | Hit targets | 36px comfortable, 28px compact (documented exception to the 44px touch ideal; 24px is the WCAG 2.2 AA floor). Coarse pointers always get 44px | 44px everywhere | Density loss |
| 13 | Icon set | Hand-drawn 16px inline SVG, 2px stroke, a handful of glyphs. No icon library | Lucide (self-hosted SVGs) | Swap `<use>` sources |
| 14 | Terminal in light theme | Real light terminal palette (ANSI white mapped to a dark grey so it stays readable) | Always dark terminal | One token group |
| 15 | Prefix | All tokens are `--th-*`, component classes `.th-*` | Unprefixed | Rename |
