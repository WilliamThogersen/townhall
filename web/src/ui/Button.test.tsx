import { createRef } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Button, type ButtonVariant } from "./Button";

const VARIANTS: readonly ButtonVariant[] = ["primary", "ghost", "danger", "danger-solid"];

describe("Button", () => {
  it("renders a default md th-btn with type button", () => {
    render(<Button>retry</Button>);
    const button = screen.getByRole("button", { name: "retry" });
    expect(button).toHaveAttribute("class", "th-btn");
    expect(button).toHaveAttribute("type", "button");
    expect(button).toBeEnabled();
  });

  it.each(VARIANTS)("adds th-btn--%s for that variant", (variant) => {
    render(<Button variant={variant}>go</Button>);
    expect(screen.getByRole("button")).toHaveAttribute("class", `th-btn th-btn--${variant}`);
  });

  it("adds no variant class for the default variant", () => {
    render(<Button variant="default">go</Button>);
    expect(screen.getByRole("button")).toHaveAttribute("class", "th-btn");
  });

  it("adds th-btn--sm for the small size only", () => {
    const { rerender } = render(<Button size="sm">go</Button>);
    expect(screen.getByRole("button")).toHaveAttribute("class", "th-btn th-btn--sm");
    rerender(<Button size="md">go</Button>);
    expect(screen.getByRole("button")).toHaveAttribute("class", "th-btn");
  });

  it("keeps the caller's class after its own", () => {
    render(
      <Button variant="ghost" size="sm" className="extra">
        go
      </Button>,
    );
    expect(screen.getByRole("button")).toHaveAttribute("class", "th-btn th-btn--ghost th-btn--sm extra");
  });

  it("keeps an explicit submit type", () => {
    render(<Button type="submit">send</Button>);
    expect(screen.getByRole("button")).toHaveAttribute("type", "submit");
  });

  it("is disabled when busy and shows a spinner and the busy label", () => {
    render(
      <Button variant="primary" busy busyLabel="starting…">
        start
      </Button>,
    );
    const button = screen.getByRole("button");
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-busy", "true");
    expect(button.querySelector(".th-spin")).toHaveAttribute("aria-hidden", "true");
    expect(button.textContent?.replace(/[⠀-⣿]/g, "")).toBe("starting…");
    expect(button).toHaveAccessibleName("starting…");
  });

  it("keeps the children when busy without a busy label", () => {
    render(<Button busy>saving</Button>);
    const button = screen.getByRole("button");
    expect(button.querySelector(".th-spin")).not.toBeNull();
    expect(button).toHaveAccessibleName("saving");
  });

  it("shows no spinner and no aria-busy at rest", () => {
    render(<Button busyLabel="starting…">start</Button>);
    const button = screen.getByRole("button");
    expect(button.querySelector(".th-spin")).toBeNull();
    expect(button).not.toHaveAttribute("aria-busy");
    expect(button).toHaveTextContent("start");
  });

  it("honours disabled", () => {
    render(<Button disabled>go</Button>);
    expect(screen.getByRole("button")).toBeDisabled();
  });

  it("does not fire clicks while busy", async () => {
    const onClick = vi.fn();
    render(
      <Button busy onClick={onClick}>
        go
      </Button>,
    );
    await userEvent.click(screen.getByRole("button"));
    expect(onClick).not.toHaveBeenCalled();
  });

  it("passes the ref to the button element", () => {
    const ref = createRef<HTMLButtonElement>();
    render(<Button ref={ref}>go</Button>);
    expect(ref.current).toBe(screen.getByRole("button"));
  });

  it("passes other props through", async () => {
    const onClick = vi.fn();
    render(
      <Button aria-label="Attach files" title="attach" data-testid="attach" onClick={onClick} autoFocus>
        +
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Attach files" });
    expect(button).toHaveAttribute("title", "attach");
    expect(button).toHaveAttribute("data-testid", "attach");
    expect(button).toHaveFocus();
    await userEvent.click(button);
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
