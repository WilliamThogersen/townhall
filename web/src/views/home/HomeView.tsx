import { useRef, useState } from "react";
import type { Repo } from "../../api/types";
import { useKeyHints, type KeyHint } from "../../shell/keyHints";
import { Banner } from "../../ui/Banner";
import { Greeting } from "./Greeting";
import type { Clock } from "./greetingText";
import { HeroComposer } from "./HeroComposer";
import { ReposKnown } from "./ReposKnown";
import { ShippedStats } from "./ShippedStats";
import { starterPrompts } from "./starterPrompts";
import { Starters } from "./Starters";
import { useComposerFocus } from "./useComposerFocus";
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
  const [draft, setDraft] = useState("");
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const focusComposer = useComposerFocus(inputRef);
  const { start, busy, error } = useStartConversation();
  const pick = (text: string) => {
    setDraft(text);
    focusComposer();
  };
  return (
    <div className="th-home">
      <div className="th-home-inner">
        <Greeting now={now} />
        {error === null ? null : <Banner level="error">{error}</Banner>}
        <HeroComposer
          value={draft}
          onChange={setDraft}
          onSubmit={start}
          busy={busy}
          inputRef={inputRef}
          tools={repos && repos.length > 0 ? <ReposKnown repos={repos} /> : null}
        />
        <Starters prompts={starterPrompts(repos)} onPick={pick} />
        <ShippedStats today={now} />
      </div>
    </div>
  );
}
