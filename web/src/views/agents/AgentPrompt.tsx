import { useState } from "react";
import { Button } from "../../ui/Button";
import { CopyButton } from "./CopyButton";
import { parseBlocks, parsePrompt, promptStats } from "./promptSections";

const UNTAGGED_LABEL = "text";

function Blocks({ text }: { readonly text: string }) {
  return (
    <>
      {parseBlocks(text).map((block, index) => {
        if (block.kind === "paragraph") {
          return (
            <p key={index} className="th-prompt-text">
              {block.text}
            </p>
          );
        }
        const List = block.ordered ? "ol" : "ul";
        return (
          <List key={index} className="th-prompt-list">
            {block.items.map((item, i) => (
              <li key={i}>{item}</li>
            ))}
          </List>
        );
      })}
    </>
  );
}

function PromptView({ prompt }: { readonly prompt: string }) {
  const parsed = parsePrompt(prompt);
  const stats = promptStats(prompt);
  const [open, setOpen] = useState<readonly boolean[]>(() => parsed.sections.map((_, index) => index === 0));
  const setAll = (value: boolean) => setOpen(parsed.sections.map(() => value));
  const toggle = (index: number) => setOpen((current) => current.map((value, i) => (i === index ? !value : value)));
  return (
    <div className="th-prompt">
      <div className="th-prompt-bar">
        <span className="th-cap">
          {`${stats.chars} chars · ~${stats.tokens} tokens · ${stats.sections} ${stats.sections === 1 ? "section" : "sections"}`}
        </span>
        <span className="th-prompt-actions">
          {parsed.sections.length > 0 ? (
            <>
              <Button size="sm" variant="ghost" onClick={() => setAll(true)}>
                expand all
              </Button>
              <Button size="sm" variant="ghost" onClick={() => setAll(false)}>
                collapse all
              </Button>
            </>
          ) : null}
          <CopyButton text={prompt} label="copy prompt" />
        </span>
      </div>
      {parsed.intro === "" ? null : (
        <div className="th-prompt-intro">
          <Blocks text={parsed.intro} />
        </div>
      )}
      {parsed.sections.map((section, index) => (
        <div key={index} className="th-prompt-section" data-open={open[index] || undefined}>
          <div className="th-prompt-section-head">
            <button type="button" className="th-prompt-toggle" aria-expanded={open[index] === true} onClick={() => toggle(index)}>
              {section.tag === "" ? UNTAGGED_LABEL : section.tag}
            </button>
            {section.closed ? null : <span className="th-cap">not closed</span>}
          </div>
          {open[index] === true ? (
            <div className="th-prompt-body">
              <Blocks text={section.body} />
            </div>
          ) : null}
        </div>
      ))}
      <p className="th-cap">read-only: edit the prompt in the toml below</p>
    </div>
  );
}

export function AgentPrompt({ prompt }: { readonly prompt: string | null }) {
  if (prompt === null || prompt.trim() === "") return <p className="th-cap">no prompt</p>;
  return <PromptView key={prompt} prompt={prompt} />;
}
