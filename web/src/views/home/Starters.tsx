import { Button } from "../../ui/Button";

export interface StartersProps {
  readonly prompts: readonly string[];
  readonly onPick: (text: string) => void;
}

export function Starters({ prompts, onPick }: StartersProps) {
  return (
    <div className="th-home-starters">
      {prompts.map((prompt) => (
        <Button key={prompt} variant="ghost" size="sm" onClick={() => onPick(prompt)}>
          {prompt}
        </Button>
      ))}
    </div>
  );
}
