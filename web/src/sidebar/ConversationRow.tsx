import { useEffect, useRef, type KeyboardEvent, type MouseEvent } from "react";
import { StatusGlyph } from "../ui/StatusGlyph";
import { ageLabel } from "./age";
import { rowRepoText } from "./conversationRepo";
import type { SidebarRow } from "./sidebarRows";

export interface MenuPoint {
  readonly x: number;
  readonly y: number;
}

export interface RowHandlers {
  readonly currentId: string | null;
  readonly now: number;
  readonly onOpen: (id: string) => void;
  readonly onMenu: (id: string, at: MenuPoint, from: HTMLElement) => void;
}

export interface ConversationRowProps {
  readonly row: SidebarRow;
  readonly showRepo: boolean;
  readonly handlers: RowHandlers;
}

const MENU_INSET_PX = 16;

function belowRow(row: HTMLElement): MenuPoint {
  const rect = row.getBoundingClientRect();
  return { x: rect.left + MENU_INSET_PX, y: rect.bottom };
}

const opensMenu = (e: KeyboardEvent) => e.key === "ContextMenu" || (e.shiftKey && e.key === "F10");

export function ConversationRow({ row, showRepo, handlers }: ConversationRowProps) {
  const { conversation, summary } = row;
  const current = conversation.id === handlers.currentId;
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (current) ref.current?.scrollIntoView?.({ block: "nearest" });
  }, [current]);

  const onContextMenu = (e: MouseEvent<HTMLButtonElement>) => {
    e.preventDefault();
    const fromKeyboard = e.clientX === 0 && e.clientY === 0;
    const at = fromKeyboard ? belowRow(e.currentTarget) : { x: e.clientX, y: e.clientY };
    handlers.onMenu(conversation.id, at, e.currentTarget);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (!opensMenu(e)) return;
    e.preventDefault();
    handlers.onMenu(conversation.id, belowRow(e.currentTarget), e.currentTarget);
  };

  return (
    <li>
      <button
        ref={ref}
        type="button"
        className="th-convo"
        data-state={summary.state ?? undefined}
        aria-current={current ? "page" : undefined}
        onClick={() => handlers.onOpen(conversation.id)}
        onContextMenu={onContextMenu}
        onKeyDown={onKeyDown}
      >
        <StatusGlyph state={summary.glyph} />
        <span className="th-convo-title" title={conversation.title}>
          {conversation.title}
        </span>
        <span className="th-convo-time">{ageLabel(conversation.updatedAt, handlers.now)}</span>
        <span className="th-convo-status">{summary.status}</span>
        <span className="th-convo-repo">{showRepo ? rowRepoText(row.repo) : ""}</span>
      </button>
    </li>
  );
}

export function RowList({ rows, showRepo, handlers }: Omit<ConversationRowProps, "row"> & { readonly rows: readonly SidebarRow[] }) {
  return (
    <ul className="th-list">
      {rows.map((row) => (
        <ConversationRow key={row.conversation.id} row={row} showRepo={showRepo} handlers={handlers} />
      ))}
    </ul>
  );
}
