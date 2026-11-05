import { memo } from "react";
import ReactMarkdown, { type Components, type ExtraProps } from "react-markdown";
import remarkBreaks from "remark-breaks";
import remarkGfm from "remark-gfm";

const SAFE_PROTOCOL = /^(https?:|mailto:)/i;
const LANGUAGE_CLASS = /^language-(.+)$/;

const REMARK_PLUGINS = [remarkGfm, remarkBreaks];

/** Only web and mail links survive. Everything else (javascript:, file:, data:, relative) loses its href. */
export function safeHref(url: string): string | null {
  const trimmed = url.trim();
  return SAFE_PROTOCOL.test(trimmed) ? trimmed : null;
}

function codeLanguage(node: ExtraProps["node"]): string | undefined {
  const code = node?.children[0];
  if (code?.type !== "element" || code.tagName !== "code") return undefined;
  const classes = code.properties.className;
  const names = Array.isArray(classes) ? classes.map(String) : [];
  return names.map((name) => LANGUAGE_CLASS.exec(name)?.[1]).find((lang) => lang !== undefined);
}

const COMPONENTS: Components = {
  a: ({ href, children }) =>
    href ? (
      <a href={href} target="_blank" rel="noreferrer noopener">
        {children}
      </a>
    ) : (
      <span>{children}</span>
    ),
  pre: ({ node, children }) => <pre data-lang={codeLanguage(node)}>{children}</pre>,
  table: ({ children }) => (
    <div className="th-md-table">
      <table>{children}</table>
    </div>
  ),
  img: ({ alt }) => (alt ? <span>{alt}</span> : null),
};

/** Renders an agent's reply as GitHub-flavored markdown. Raw HTML is shown as text, never rendered. */
export const Markdown = memo(function Markdown({ text }: { readonly text: string }) {
  return (
    <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={COMPONENTS} urlTransform={safeHref}>
      {text}
    </ReactMarkdown>
  );
});
