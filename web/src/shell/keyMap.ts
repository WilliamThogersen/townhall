import { FILTER_HINT, FOLD_HINT, OPEN_HINT, ROWS_HINT } from "../views/control/useControlKeys";
import { REPLY_HINT, SESSION_HINT } from "../views/conversation/useConversationKeys";
import { CLOSE_TERMINAL_HINT } from "../views/conversation/session/useCloseKey";
import { OPEN_TASK_HINT, TASK_ROWS_HINT } from "../views/conversation/session/useTaskKeys";
import { ADD_REPO_HINT } from "../views/repos/useRepoKeys";
import { HELP_HINT, NEW_CONVERSATION_HINT, SEARCH_HINT } from "./globalKeys";
import type { KeyHint } from "./keyHints";

export interface KeyGroup {
  readonly title: string;
  readonly entries: readonly KeyHint[];
}

export const AGENT_LIST_HINTS: readonly KeyHint[] = [
  { keys: "↑↓", label: "agent types" },
  { keys: "esc", label: "clear filter" },
];

export const PLUGINS_HINTS: readonly KeyHint[] = [{ keys: "tab", label: "next control" }];
export const PLUGIN_PROMPT_HINTS: readonly KeyHint[] = [{ keys: "esc", label: "cancel install" }];

export const KEY_MAP: readonly KeyGroup[] = [
  {
    title: "anywhere",
    entries: [
      { keys: NEW_CONVERSATION_HINT.keys, label: "home, composer focused" },
      { keys: SEARCH_HINT.keys, label: "search" },
      { keys: "⌥↑ ⌥↓", label: "previous or next conversation" },
      { keys: HELP_HINT.keys, label: "this help" },
      { keys: "esc", label: "leave a field, close a menu, cancel a confirm" },
    ],
  },
  {
    title: "home",
    entries: [
      { keys: "⏎", label: "start the conversation" },
      { keys: "⇧⏎", label: "new line" },
    ],
  },
  {
    title: "conversation",
    entries: [
      { keys: REPLY_HINT.keys, label: "focus the composer" },
      { keys: SESSION_HINT.keys, label: "fold or unfold the session panel" },
      { keys: "⏎", label: "send" },
      { keys: "⇧⏎", label: "new line" },
      { keys: "tab", label: "move between thread, task list, composer and open panes" },
      { keys: "← → esc", label: "in the lightbox: step through images, close" },
    ],
  },
  {
    title: "session panel",
    entries: [
      { keys: TASK_ROWS_HINT.keys, label: "move through the tasks" },
      { keys: OPEN_TASK_HINT.keys, label: "open the task's terminal" },
    ],
  },
  {
    title: "terminal",
    entries: [
      { keys: CLOSE_TERMINAL_HINT.keys, label: "close the terminal when it is not focused" },
      { keys: "esc", label: "interrupt claude while the terminal has focus" },
    ],
  },
  {
    title: "control center",
    entries: [
      { keys: ROWS_HINT.keys, label: "move between rows" },
      { keys: FOLD_HINT.keys, label: "fold and unfold" },
      { keys: OPEN_HINT.keys, label: "open the conversation" },
      { keys: FILTER_HINT.keys, label: "filter by repo" },
    ],
  },
  {
    title: "repos",
    entries: [{ keys: ADD_REPO_HINT.keys, label: ADD_REPO_HINT.label }],
  },
  {
    title: "agents",
    entries: [
      { keys: "↑↓", label: "move between agent types" },
      { keys: "esc", label: "clear the filter" },
    ],
  },
  {
    title: "plugins",
    entries: [
      { keys: "tab", label: "next control" },
      { keys: "esc", label: "cancel an install" },
    ],
  },
  {
    title: "settings",
    entries: [
      { keys: "tab", label: "next setting" },
      { keys: "← →", label: "change a choice" },
    ],
  },
];
