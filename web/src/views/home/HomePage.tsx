import { useContributions, useRepos } from "../../data";
import { FirstRunSlot } from "./FirstRunSlot";
import { systemClock, type Clock } from "./greetingText";
import { HomeView } from "./HomeView";

export interface HomePageProps {
  readonly clock?: Clock;
}

export function HomePage({ clock = systemClock }: HomePageProps) {
  const { repos, loading } = useRepos();
  const { starters } = useContributions();
  if (repos === null && loading) return <div className="th-home" />;
  if (repos !== null && repos.length === 0) return <FirstRunSlot />;
  return <HomeView repos={repos} clock={clock} pluginStarters={starters} />;
}
