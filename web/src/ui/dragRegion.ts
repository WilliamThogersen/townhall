/**
 * Spread on a bar to let the desktop window be dragged by it. "deep" covers the whole subtree;
 * Tauri still skips buttons, links and inputs, so they keep working. A no-op in the browser.
 */
export const DRAG_REGION = { "data-tauri-drag-region": "deep" } as const;
