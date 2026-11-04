import { useRepos } from "../../data";
import { SEARCH_HINT } from "../../shell/globalKeys";
import { useKeyHints, type KeyHint } from "../../shell/keyHints";
import { FirstRunSlot } from "./FirstRunSlot";
import { systemClock, type Clock } from "./greetingText";
import { HomeView } from "./HomeView";

const FIRST_RUN_KEYS: readonly KeyHint[] = [SEARCH_HINT];

function FirstRun() {
  useKeyHints(FIRST_RUN_KEYS);
  return <FirstRunSlot />;
}

export interface HomePageProps {
  readonly clock?: Clock;
}

export function HomePage({ clock = systemClock }: HomePageProps) {
  const { repos, loading } = useRepos();
  if (repos === null && loading) return <div className="th-home" />;
  if (repos !== null && repos.length === 0) return <FirstRun />;
  return <HomeView repos={repos} clock={clock} />;
}
