import type { ButtonHTMLAttributes, Ref } from "react";
import { Spinner } from "./Spinner";

export type ButtonVariant = "default" | "primary" | "ghost" | "danger" | "danger-solid";

export type ButtonSize = "md" | "sm";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  readonly variant?: ButtonVariant;
  readonly size?: ButtonSize;
  readonly busy?: boolean;
  readonly busyLabel?: string;
  readonly ref?: Ref<HTMLButtonElement>;
}

function buttonClassName(variant: ButtonVariant, size: ButtonSize, extra: string | undefined): string {
  return [
    "th-btn",
    variant === "default" ? null : `th-btn--${variant}`,
    size === "sm" ? "th-btn--sm" : null,
    extra || null,
  ]
    .filter((name) => name !== null)
    .join(" ");
}

export function Button({
  variant = "default",
  size = "md",
  busy = false,
  busyLabel,
  className,
  type = "button",
  disabled,
  children,
  ref,
  ...rest
}: ButtonProps) {
  return (
    <button
      {...rest}
      ref={ref}
      type={type}
      className={buttonClassName(variant, size, className)}
      disabled={busy || disabled}
      aria-busy={busy || undefined}
    >
      {busy ? (
        <>
          <Spinner />
          {busyLabel ?? children}
        </>
      ) : (
        children
      )}
    </button>
  );
}
