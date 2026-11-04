import { useRef, useState } from "react";
import type { Repo } from "../../api/types";
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
}

export function HomeView({ repos, clock }: HomeViewProps) {
  useKeyHints(HOME_KEYS);
  const [now] = useState(clock);
  const composer = useRef<ComposerHandle>(null);
  useFocusOnNewKey(composer);
  const start = useStartConversation();
  return (
    <div className="th-home">
      <div className="th-home-inner">
        <Greeting now={now} />
        <Composer
          variant="hero"
          ref={composer}
          autoFocus
          onSubmit={start}
          tools={repos && repos.length > 0 ? <ReposKnown repos={repos} /> : null}
        />
        <Starters prompts={starterPrompts(repos)} onPick={(text) => composer.current?.fill(text)} />
        <ShippedStats today={now} />
      </div>
    </div>
  );
}
