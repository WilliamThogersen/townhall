import type { AnchorHTMLAttributes, MouseEvent, Ref } from "react";
import { navigate } from "./history";

export type LinkProps = Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> & {
  readonly to: string;
  readonly replace?: boolean;
  readonly ref?: Ref<HTMLAnchorElement>;
};

function isPlainLeftClick(e: MouseEvent<HTMLAnchorElement>): boolean {
  return e.button === 0 && !e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey;
}

export function Link({ to, replace, onClick, target, ...rest }: LinkProps) {
  const handleClick = (e: MouseEvent<HTMLAnchorElement>) => {
    onClick?.(e);
    if (e.defaultPrevented || !isPlainLeftClick(e)) return;
    if (target && target !== "_self") return;
    e.preventDefault();
    navigate(to, { replace });
  };
  return <a {...rest} href={to} target={target} onClick={handleClick} />;
}
