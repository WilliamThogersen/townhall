import { Fragment } from "react";
import { RowList, type RowHandlers } from "./ConversationRow";
import type { GroupBlock } from "./sidebarRows";

export interface RepoGroupsProps {
  readonly blocks: readonly GroupBlock[];
  readonly onToggle: (key: string) => void;
  readonly handlers: RowHandlers;
}

export function RepoGroups({ blocks, onToggle, handlers }: RepoGroupsProps) {
  return blocks.map((block) => (
    <Fragment key={block.key}>
      <button
        type="button"
        className="th-group-head"
        aria-expanded={block.open}
        aria-label={`${block.label} ${block.count}`}
        onClick={() => onToggle(block.key)}
      >
        <span>{block.label}</span>
        <span>{block.count}</span>
      </button>
      <RowList rows={block.rows} showRepo={false} handlers={handlers} />
    </Fragment>
  ));
}
