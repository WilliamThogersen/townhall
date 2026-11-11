import type { PluginStarter } from "../../api/types";
import { Button } from "../../ui/Button";

export interface StartersProps {
  readonly prompts: readonly string[];
  readonly extra?: readonly PluginStarter[];
  readonly onPick: (text: string) => void;
}

export function Starters({ prompts, extra = [], onPick }: StartersProps) {
  return (
    <div className="th-home-starters">
      {prompts.map((prompt) => (
        <Button key={prompt} variant="ghost" size="sm" onClick={() => onPick(prompt)}>
          {prompt}
        </Button>
      ))}
      {extra.map((starter, index) => (
        <Button
          key={`${starter.plugin}:${index}`}
          variant="ghost"
          size="sm"
          title={`from plugin ${starter.plugin}`}
          onClick={() => onPick(starter.text)}
        >
          {starter.title}
        </Button>
      ))}
    </div>
  );
}
