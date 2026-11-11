import type { QueuedMessage } from "./queue";

export interface QueueListProps {
  readonly items: readonly QueuedMessage[];
  readonly onRemove: (id: number) => void;
}

function label(item: QueuedMessage): string {
  if (item.text !== "") return item.text;
  return item.attachmentIds.length === 1 ? "1 attachment" : `${item.attachmentIds.length} attachments`;
}

export function QueueList({ items, onRemove }: QueueListProps) {
  if (items.length === 0) return null;
  return (
    <ul className="th-queue" aria-label="Queued messages">
      {items.map((item) => (
        <li key={item.id} className="th-queue-row">
          <span className="th-queue-text" title={label(item)}>
            {label(item)}
          </span>
          <button
            type="button"
            className="th-queue-remove"
            aria-label={`Remove queued message: ${label(item)}`}
            onClick={() => onRemove(item.id)}
          >
            ×
          </button>
        </li>
      ))}
    </ul>
  );
}
