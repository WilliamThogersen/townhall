import { useEffect, useState } from "react";
import { Button } from "../../ui/Button";

const COPIED_MS = 1500;

export function CopyButton({ text, label = "copy" }: { readonly text: string; readonly label?: string }) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), COPIED_MS);
    return () => clearTimeout(timer);
  }, [copied]);
  const copy = () => {
    navigator.clipboard.writeText(text).then(
      () => setCopied(true),
      () => setCopied(false),
    );
  };
  return (
    <Button size="sm" variant="ghost" onClick={copy} data-copied={copied || undefined}>
      {copied ? "copied" : label}
    </Button>
  );
}
