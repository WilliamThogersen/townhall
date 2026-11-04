import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider, useToasts, type ToastInput } from "./Toasts";

type Api = ReturnType<typeof useToasts>;

function Capture({ onApi }: { onApi: (api: Api) => void }) {
  onApi(useToasts());
  return <p>app</p>;
}

function setup() {
  const ref: { api: Api | null } = { api: null };
  render(
    <ToastProvider>
      <Capture onApi={(api) => (ref.api = api)} />
    </ToastProvider>,
  );
  const api = () => {
    if (!ref.api) throw new Error("no api");
    return ref.api;
  };
  const push = (input: ToastInput) => {
    let id = "";
    act(() => {
      id = api().push(input);
    });
    return id;
  };
  return { api, push };
}

const toast = (title: string, extra: Partial<ToastInput> = {}): ToastInput => ({
  title,
  text: "needs you",
  ...extra,
});

function region() {
  return screen.getByRole("status");
}

function shown() {
  return Array.from(region().querySelectorAll(".th-toast"));
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("ToastProvider", () => {
  it("renders children and an empty status container", () => {
    setup();
    expect(screen.getByText("app")).toBeInTheDocument();
    expect(region()).toHaveClass("th-toasts");
    expect(shown()).toHaveLength(0);
  });

  it("shows a pushed toast with title, text and context", () => {
    const { push } = setup();
    push(toast("docs-sweep", { context: "docs sweep" }));
    expect(shown()).toHaveLength(1);
    const title = within(region()).getByText("docs-sweep");
    expect(title.tagName).toBe("B");
    expect(title.parentElement).toHaveTextContent("docs-sweep needs you · docs sweep");
  });

  it("leaves out the context separator without a context", () => {
    const { push } = setup();
    push(toast("query-plan", { text: "ended" }));
    expect(within(region()).getByText("query-plan").parentElement?.textContent).toBe("query-plan ended");
  });

  it("renders the glyph first, unwrapped", () => {
    const { push } = setup();
    push(toast("e2e-smoke", { glyph: <span className="th-status" data-state="error" aria-hidden="true" /> }));
    const first = shown()[0]?.firstElementChild;
    expect(first).toHaveClass("th-status");
    expect(first).toHaveAttribute("data-state", "error");
  });

  it("returns distinct ids", () => {
    const { push } = setup();
    const a = push(toast("a"));
    const b = push(toast("b"));
    expect(a).not.toBe(b);
  });

  it("keeps only the newest three, newest last", () => {
    const { push } = setup();
    ["one", "two", "three", "four"].forEach((title) => push(toast(title)));
    expect(shown()).toHaveLength(3);
    expect(within(region()).queryByText("one")).toBeNull();
    expect(shown().map((node) => node.querySelector("b")?.textContent)).toEqual(["two", "three", "four"]);
  });

  it("removes a toast after 6 seconds", () => {
    const { push } = setup();
    push(toast("docs-sweep"));
    act(() => vi.advanceTimersByTime(5999));
    expect(shown()).toHaveLength(1);
    act(() => vi.advanceTimersByTime(1));
    expect(shown()).toHaveLength(0);
  });

  it("pauses while hovered and resumes with the time left", () => {
    const { push } = setup();
    push(toast("docs-sweep"));
    act(() => vi.advanceTimersByTime(4000));
    const node = shown()[0] as HTMLElement;
    fireEvent.mouseEnter(node);
    act(() => vi.advanceTimersByTime(10_000));
    expect(shown()).toHaveLength(1);
    fireEvent.mouseLeave(node);
    act(() => vi.advanceTimersByTime(1999));
    expect(shown()).toHaveLength(1);
    act(() => vi.advanceTimersByTime(1));
    expect(shown()).toHaveLength(0);
  });

  it("pauses while focus is inside a toast", () => {
    const { push } = setup();
    push(toast("docs-sweep", { action: { label: "open", onClick: vi.fn() } }));
    const button = within(region()).getByRole("button", { name: "open" });
    act(() => button.focus());
    act(() => vi.advanceTimersByTime(10_000));
    expect(shown()).toHaveLength(1);
    act(() => button.blur());
    act(() => vi.advanceTimersByTime(6000));
    expect(shown()).toHaveLength(0);
  });

  it("dismisses with the × button when there is no action", () => {
    const { push } = setup();
    push(toast("query-plan"));
    const dismiss = within(region()).getByRole("button", { name: "Dismiss" });
    expect(dismiss).toHaveTextContent("×");
    expect(dismiss).toHaveClass("th-btn", "th-btn--ghost", "th-btn--sm");
    fireEvent.click(dismiss);
    expect(shown()).toHaveLength(0);
  });

  it("dismisses by id", () => {
    const { api, push } = setup();
    const id = push(toast("a"));
    push(toast("b"));
    act(() => api().dismiss(id));
    expect(shown().map((node) => node.querySelector("b")?.textContent)).toEqual(["b"]);
  });

  it("ignores an unknown id", () => {
    const { api, push } = setup();
    push(toast("a"));
    act(() => api().dismiss("nope"));
    expect(shown()).toHaveLength(1);
  });

  it("renders the action instead of dismiss and calls it on click", () => {
    const onClick = vi.fn();
    const { push } = setup();
    push(toast("docs-sweep", { action: { label: "open", onClick } }));
    expect(within(region()).queryByRole("button", { name: "Dismiss" })).toBeNull();
    const action = within(region()).getByRole("button", { name: "open" });
    expect(action).toHaveClass("th-btn", "th-btn--ghost", "th-btn--sm");
    expect(action).toHaveAttribute("type", "button");
    fireEvent.click(action);
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("closes the toast after its action runs", () => {
    const { push } = setup();
    push(toast("docs-sweep", { action: { label: "open", onClick: vi.fn() } }));
    fireEvent.click(within(region()).getByRole("button", { name: "open" }));
    expect(shown()).toHaveLength(0);
  });
});

describe("useToasts", () => {
  it("throws outside a ToastProvider", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    expect(() => render(<Capture onApi={() => undefined} />)).toThrow(/ToastProvider/);
    vi.mocked(console.error).mockRestore();
  });
});
