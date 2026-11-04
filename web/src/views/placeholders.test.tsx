import { fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it } from "vitest";
import { KeyHintsProvider, useCurrentKeyHints } from "../shell/keyHints";
import {
  ConversationPlaceholder,
  HomePlaceholder,
  NotFound,
  SearchPlaceholder,
} from "./placeholders";

function Hints() {
  return <output>{useCurrentKeyHints().map((h) => h.keys).join(" ")}</output>;
}

function show(view: ReactNode) {
  return render(<KeyHintsProvider>{view}<Hints /></KeyHintsProvider>);
}

const title = (name: string) => screen.getByRole("heading", { name });
const hints = () => screen.getByRole("status").textContent;

beforeEach(() => window.history.replaceState(null, "", "/"));

describe("placeholders", () => {
  it("home points at the control center", () => {
    show(<HomePlaceholder />);
    expect(title("conversations come next")).toBeInTheDocument();
    expect(screen.getByText("start agents from the control center for now")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: "open control center" }));
    expect(window.location.pathname).toBe("/townhall");
    expect(hints()).toBe("/");
  });

  it("a conversation names its id", () => {
    show(<ConversationPlaceholder id="fix-login" />);
    expect(title("conversations come next")).toBeInTheDocument();
    expect(screen.getByText("conversation fix-login will open here")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "open control center" })).toHaveAttribute("href", "/townhall");
    expect(hints()).toBe("n /");
  });

  it.each([
    ["search", <SearchPlaceholder />, "search comes next", "there are no conversations to search yet", "n"],
  ])("%s says what comes next", (_, view, heading, body, keys) => {
    show(view);
    expect(title(heading)).toBeInTheDocument();
    expect(screen.getByText(body)).toBeInTheDocument();
    expect(hints()).toBe(keys);
  });

  it("not found names the path and goes home", () => {
    window.history.replaceState(null, "", "/nope");
    show(<NotFound path="/nope" />);
    expect(title("not found")).toBeInTheDocument();
    expect(screen.getByText("nothing lives at /nope")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: "go home" }));
    expect(window.location.pathname).toBe("/");
  });
});
