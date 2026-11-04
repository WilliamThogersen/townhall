# townhall docs

Design documentation. Nothing here changes the running app: `style-guide/tokens.css` is not imported by `web/` yet.

| File | What it covers |
|------|----------------|
| [style-guide/foundations.md](style-guide/foundations.md) | Principles, color, type, spacing, radii, elevation, motion, icons, focus, z-index |
| [style-guide/components.md](style-guide/components.md) | App shell, tabs, button, icon button, inputs, select, form field |
| [style-guide/components-agents.md](style-guide/components-agents.md) | Status indicator, badge, agent card, terminal panel |
| [style-guide/components-feedback.md](style-guide/components-feedback.md) | Confirm dialog, toast, error banner, empty state, loading skeleton |
| [style-guide/activity-page.md](style-guide/activity-page.md) | Planned Activity page (route `/`): feed anatomy, grouping, filters, density, 50 agents |
| [style-guide/layout.md](style-guide/layout.md) | Page layout, agent grid, breakpoints, density modes, desktop (Tauri) specifics |
| [style-guide/voice.md](style-guide/voice.md) | Microcopy rules and examples |
| [style-guide/tokens.css](style-guide/tokens.css) | The token file, single source of truth (every `--th-*` token in these docs exists here) |
| [style-guide/preview.html](style-guide/preview.html) | Open in a browser: every token and component state, theme and density toggles |
| [style-guide/check-contrast.mjs](style-guide/check-contrast.mjs) | `node docs/style-guide/check-contrast.mjs` verifies WCAG ratios from tokens.css |
| [decisions.md](decisions.md) | Open design decisions made on the owner's behalf, for veto |

Start with `preview.html`, then `decisions.md`.
