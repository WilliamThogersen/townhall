import { useState } from "react";
import { PluginSlot } from "../plugins/PluginSlot";
import { useUiEntries } from "../plugins/useUiEntries";
import { Button } from "../ui/Button";

export interface PluginComposerActionsProps {
  readonly conversationId: string | null;
  readonly onInsert: (text: string) => void;
}

export function PluginComposerActions({ conversationId, onInsert }: PluginComposerActionsProps) {
  const entries = useUiEntries("composer.action");
  const [open, setOpen] = useState<string | null>(null);
  return (
    <>
      {entries.map((entry) => {
        const key = `${entry.plugin}/${entry.entry}`;
        const isOpen = open === key;
        return (
          <span key={key} className="th-composer-action">
            <Button variant="ghost" size="sm" aria-expanded={isOpen} onClick={() => setOpen(isOpen ? null : key)}>
              {entry.title}
            </Button>
            {isOpen ? (
              <div className="th-composer-popover" role="group" aria-label={entry.title}>
                <PluginSlot name="composer.action" only={entry} context={{ conversationId }} onInsert={onInsert} />
              </div>
            ) : null}
          </span>
        );
      })}
    </>
  );
}
