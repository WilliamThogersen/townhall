import type { ReplyTarget } from "./replies";

export interface ReplyBarProps {
  readonly target: ReplyTarget;
  readonly onCancel: () => void;
}

/** The entry being answered, shown above the input. */
export function ReplyBar({ target, onCancel }: ReplyBarProps) {
  return (
    <div className="th-reply-bar" role="group" aria-label="Replying to">
      <span className="th-reply-author">{target.author}</span>
      <span className="th-reply-excerpt">{target.excerpt}</span>
      <button type="button" className="th-reply-cancel" aria-label="Cancel reply" onClick={onCancel}>
        ×
      </button>
    </div>
  );
}
