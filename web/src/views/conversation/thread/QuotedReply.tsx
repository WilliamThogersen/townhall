import type { MouseEvent } from "react";
import type { ReplyTarget } from "../../../composer/replies";

const FLASH_MS = 1500;
const flashing = new Map<Element, number>();

function prefersReducedMotion(): boolean {
  return typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function showEntry(from: Element, id: number): void {
  const original = from.closest(".th-thread")?.querySelector(`[data-entry-id="${id}"]`);
  if (original === null || original === undefined) return;
  original.scrollIntoView({ block: "center", behavior: prefersReducedMotion() ? "auto" : "smooth" });
  window.clearTimeout(flashing.get(original));
  original.setAttribute("data-flash", "true");
  flashing.set(
    original,
    window.setTimeout(() => {
      original.removeAttribute("data-flash");
      flashing.delete(original);
    }, FLASH_MS),
  );
}

export interface QuotedReplyProps {
  readonly id: number;
  readonly target: ReplyTarget | null;
}

export function QuotedReply({ id, target }: QuotedReplyProps) {
  if (target === null) return <p className="th-msg-quote th-msg-quote--gone">original message not available</p>;
  return (
    <button
      type="button"
      className="th-msg-quote"
      aria-label="Go to the message you replied to"
      onClick={(event: MouseEvent<HTMLButtonElement>) => showEntry(event.currentTarget, id)}
    >
      <span className="th-reply-author">{target.author}</span>
      <span className="th-reply-excerpt">{target.excerpt}</span>
    </button>
  );
}
