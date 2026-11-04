import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { renderWithHints, shownHints } from "../../test/renderWithHints";
import { ConversationPage } from "./ConversationPage";

beforeEach(() => window.history.replaceState(null, "", "/"));

describe("ConversationPage", () => {
  it("names its id", () => {
    renderWithHints(<ConversationPage id="fix-login" />);
    expect(screen.getByRole("heading", { name: "conversations come next" })).toBeInTheDocument();
    expect(screen.getByText("conversation fix-login will open here")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "open control center" })).toHaveAttribute("href", "/townhall");
    expect(shownHints()).toBe("n /");
  });
});
