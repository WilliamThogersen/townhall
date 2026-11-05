import type { ButtonVariant } from "../ui/Button";

export type ComposerVariant = "hero" | "thread";

export interface ComposerLook {
  readonly className: string;
  readonly rows: number;
  readonly placeholder: string;
  readonly attachPlaceholder: string;
  readonly ariaLabel: string;
  readonly submitLabel: string;
  readonly busyLabel: string;
  readonly button: ButtonVariant;
}

export const THINKING_PLACEHOLDER = "townhall is thinking…";

export const LOOKS: Readonly<Record<ComposerVariant, ComposerLook>> = {
  hero: {
    className: "th-composer th-composer--hero",
    rows: 3,
    placeholder: "what should we work on? name a repo, or describe the code",
    attachPlaceholder: "what should we work on? name a repo, or describe the code",
    ariaLabel: "Describe the work",
    submitLabel: "start",
    busyLabel: "starting…",
    button: "primary",
  },
  thread: {
    className: "th-composer",
    rows: 2,
    placeholder: "reply, or ask for more work",
    attachPlaceholder: "reply, ask for more work, or drop files here",
    ariaLabel: "Message",
    submitLabel: "send",
    busyLabel: "sending…",
    button: "default",
  },
};

export function helpText(submitLabel: string, attachments: boolean): string {
  const keys = `⏎ ${submitLabel} · ⇧⏎ new line`;
  return attachments ? `${keys} · paste or drop images` : keys;
}
