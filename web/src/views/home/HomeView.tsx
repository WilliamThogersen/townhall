import { useRef, useState } from "react";
import type { PluginStarter, Repo } from "../../api/types";
import { DropZone, useFileDrop } from "../../attachments";
import { Composer, type ComposerHandle } from "../../composer";
import { useKeyHints, type KeyHint } from "../../shell/keyHints";
import { Greeting } from "./Greeting";
import type { Clock } from "./greetingText";
import { ReposKnown } from "./ReposKnown";
import { ShippedStats } from "./ShippedStats";
import { starterPrompts } from "./starterPrompts";
import { Starters } from "./Starters";
import { useFocusOnNewKey } from "./useFocusOnNewKey";
import { useStartConversation } from "./useStartConversation";

export const HOME_KEYS: readonly KeyHint[] = [
  { keys: "⏎", label: "start" },
  { keys: "⇧⏎", label: "new line" },
  { keys: "esc", label: "leave input" },
];

export interface HomeViewProps {
  readonly repos: readonly Repo[] | null;
  readonly clock: Clock;
  readonly pluginStarters?: readonly PluginStarter[];
}

export function HomeView({ repos, clock, pluginStarters }: HomeViewProps) {
  useKeyHints(HOME_KEYS);
  const [now] = useState(clock);
  const composer = useRef<ComposerHandle>(null);
  useFocusOnNewKey(composer);
  const start = useStartConversation();
  const drop = useFileDrop((files) => composer.current?.addFiles(files));
  return (
    <div className="th-home" {...drop.dropProps}>
      {drop.dragging ? <DropZone /> : null}
      <div className="th-home-inner">
        <Greeting now={now} />
        <Composer
          variant="hero"
          ref={composer}
          autoFocus
          stageFiles
          onSubmit={start}
          tools={repos && repos.length > 0 ? <ReposKnown repos={repos} /> : null}
        />
        <Starters
          prompts={starterPrompts(repos)}
          extra={pluginStarters}
          onPick={(text) => composer.current?.fill(text)}
        />
        <ShippedStats today={now} />
      </div>
    </div>
  );
}
