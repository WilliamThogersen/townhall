import { Fragment, useMemo } from "react";
import { parseBlocks, type Block, type Inline, type Line } from "./parseMarkdown";

function InlineView({ part }: { readonly part: Inline }) {
  switch (part.kind) {
    case "text":
      return <>{part.text}</>;
    case "code":
      return <code>{part.text}</code>;
    case "strong":
      return (
        <strong>
          <Inlines parts={part.children} />
        </strong>
      );
    case "em":
      return (
        <em>
          <Inlines parts={part.children} />
        </em>
      );
    case "link":
      return (
        <a href={part.href} target="_blank" rel="noreferrer noopener">
          <Inlines parts={part.children} />
        </a>
      );
  }
}

function Inlines({ parts }: { readonly parts: readonly Inline[] }) {
  return (
    <>
      {parts.map((part, i) => (
        <InlineView key={i} part={part} />
      ))}
    </>
  );
}

function Lines({ lines }: { readonly lines: readonly Line[] }) {
  return (
    <>
      {lines.map((line, i) => (
        <Fragment key={i}>
          {i > 0 ? <br /> : null}
          <Inlines parts={line} />
        </Fragment>
      ))}
    </>
  );
}

function BlockView({ block }: { readonly block: Block }) {
  switch (block.kind) {
    case "paragraph":
      return (
        <p>
          <Lines lines={block.lines} />
        </p>
      );
    case "code":
      return (
        <pre data-lang={block.lang || undefined}>
          <code>{block.text}</code>
        </pre>
      );
    case "list": {
      const items = block.items.map((item, i) => (
        <li key={i}>
          <Lines lines={item} />
        </li>
      ));
      return block.ordered ? <ol start={block.start === 1 ? undefined : block.start}>{items}</ol> : <ul>{items}</ul>;
    }
  }
}

export function Markdown({ text }: { readonly text: string }) {
  const blocks = useMemo(() => parseBlocks(text), [text]);
  return (
    <>
      {blocks.map((block, i) => (
        <BlockView key={i} block={block} />
      ))}
    </>
  );
}
