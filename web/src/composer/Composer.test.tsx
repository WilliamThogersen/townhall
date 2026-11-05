import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import { Composer, type ComposerHandle } from "./Composer";

const input = () => screen.getByRole("textbox");

function deferred() {
  let resolve: () => void = () => undefined;
  let reject: (error: Error) => void = () => undefined;
  const promise = new Promise<void>((ok, fail) => {
    resolve = ok;
    reject = fail;
  });
  return { promise, resolve, reject };
}

describe("Composer hero", () => {
  it("matches the home composer markup", () => {
    const { container } = render(
      <Composer variant="hero" onSubmit={vi.fn()} tools={<a className="th-repos-known" href="/repos">4 repos known</a>} />,
    );
    const form = container.querySelector("form");
    expect(form).toHaveClass("th-composer th-composer--hero");
    const box = form?.querySelector(":scope > .th-composer-box");
    expect(box?.querySelector(".th-composer-sign")).toHaveTextContent("›");
    expect(box?.querySelector(".th-composer-sign")).toHaveAttribute("aria-hidden", "true");
    expect(input()).toHaveClass("th-composer-input");
    expect(input()).toHaveAttribute("rows", "3");
    expect(input()).toHaveAttribute("placeholder", "what should we work on? name a repo, or describe the code");
    expect(input()).toHaveAccessibleName("Describe the work");
    const tools = box?.querySelector(".th-composer-tools") as HTMLElement;
    expect(tools.children[0]).toHaveTextContent("4 repos known");
    expect(tools.querySelector(".th-help")).toHaveTextContent("⏎ start · ⇧⏎ new line");
    const start = screen.getByRole("button", { name: "start" });
    expect(start).toHaveClass("th-btn th-btn--primary th-btn--sm");
    expect(start).toHaveAttribute("type", "submit");
    expect(tools.lastElementChild).toBe(start);
    expect(start).toBeDisabled();
    expect(screen.queryByRole("button", { name: "Attach files or images" })).not.toBeInTheDocument();
  });

  it("starts on enter with the trimmed text and clears it once that worked", async () => {
    const user = userEvent.setup();
    const pending = deferred();
    const onSubmit = vi.fn(() => pending.promise);
    render(<Composer variant="hero" onSubmit={onSubmit} />);
    await user.type(input(), "  fix the login loop  {Enter}");
    expect(onSubmit).toHaveBeenCalledWith("fix the login loop", [], []);
    expect(screen.getByRole("button", { name: "starting…" })).toBeDisabled();
    expect(input()).toHaveValue("  fix the login loop  ");
    expect(input()).toHaveAttribute("readonly");
    await act(async () => pending.resolve());
    expect(input()).toHaveValue("");
    expect(screen.getByRole("button", { name: "start" })).toBeDisabled();
  });

  it("starts from the button too", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<Composer variant="hero" onSubmit={onSubmit} />);
    await user.type(input(), "find slow queries");
    await user.click(screen.getByRole("button", { name: "start" }));
    expect(onSubmit).toHaveBeenCalledWith("find slow queries", [], []);
  });

  it("adds a line on shift enter", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<Composer variant="hero" onSubmit={onSubmit} />);
    await user.type(input(), "one{Shift>}{Enter}{/Shift}two");
    expect(input()).toHaveValue("one\ntwo");
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("never sends empty or blank text", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<Composer variant="hero" onSubmit={onSubmit} />);
    await user.type(input(), "   {Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "start" })).toBeDisabled();
  });

  it("keeps the text and says why when sending fails", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockRejectedValue(new Error("cannot reach the daemon"));
    const { container } = render(<Composer variant="hero" onSubmit={onSubmit} />);
    await user.type(input(), "fix it{Enter}");
    const help = container.querySelector(".th-help");
    expect(help).toHaveTextContent("error: cannot reach the daemon");
    expect(help).toHaveAttribute("data-tone", "error");
    expect(input()).toHaveValue("fix it");
    await user.type(input(), "!");
    expect(container.querySelector(".th-help")).toHaveTextContent("⏎ start · ⇧⏎ new line");
  });

  it("does not send while the input is in an input method composition", () => {
    const onSubmit = vi.fn();
    render(<Composer variant="hero" onSubmit={onSubmit} />);
    fireEvent.change(input(), { target: { value: "にほん" } });
    fireEvent.keyDown(input(), { key: "Enter", isComposing: true });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("lets go of focus on esc", async () => {
    const user = userEvent.setup();
    render(<Composer variant="hero" onSubmit={vi.fn()} autoFocus />);
    expect(input()).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(input()).not.toHaveFocus();
  });

  it("can be disabled", () => {
    render(<Composer variant="hero" onSubmit={vi.fn()} disabled />);
    expect(input()).toBeDisabled();
    expect(screen.getByRole("button", { name: "start" })).toBeDisabled();
  });

  it("takes a custom label, placeholder and button", () => {
    render(<Composer onSubmit={vi.fn()} placeholder="say more" ariaLabel="Reply" submitLabel="reply" />);
    expect(screen.getByRole("textbox", { name: "Reply" })).toHaveAttribute("placeholder", "say more");
    expect(screen.getByRole("button", { name: "reply" })).toBeInTheDocument();
  });
});

describe("Composer focus", () => {
  it("focuses again whenever the focus key changes", () => {
    const { rerender } = render(<Composer onSubmit={vi.fn()} focusKey={1} />);
    expect(input()).not.toHaveFocus();
    rerender(<Composer onSubmit={vi.fn()} focusKey={2} />);
    expect(input()).toHaveFocus();
  });

  it("gives the parent focus and fill through its ref", () => {
    const ref = createRef<ComposerHandle>();
    render(<Composer variant="hero" onSubmit={vi.fn()} ref={ref} />);
    act(() => ref.current?.fill("townhall needs a settings page"));
    expect(input()).toHaveValue("townhall needs a settings page");
    expect(input()).toHaveFocus();
    input().blur();
    act(() => ref.current?.focus());
    expect(input()).toHaveFocus();
  });
});

describe("Composer while townhall thinks", () => {
  it("disables the input and turns the button into stop", async () => {
    const user = userEvent.setup();
    const onStop = vi.fn();
    render(<Composer onSubmit={vi.fn()} busy onStop={onStop} />);
    expect(input()).toBeDisabled();
    expect(input()).toHaveAttribute("placeholder", "townhall is thinking…");
    const stop = screen.getByRole("button", { name: "stop" });
    expect(stop).toHaveClass("th-btn th-btn--danger th-btn--sm");
    expect(stop).toHaveAttribute("type", "button");
    await user.click(stop);
    expect(onStop).toHaveBeenCalledOnce();
  });

  it("keeps a disabled send without a stop handler", () => {
    render(<Composer onSubmit={vi.fn()} busy />);
    expect(screen.getByRole("button", { name: "send" })).toBeDisabled();
  });
});
