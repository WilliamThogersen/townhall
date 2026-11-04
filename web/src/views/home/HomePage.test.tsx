import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { renderWithHints, shownHints } from "../../test/renderWithHints";
import { HomePage } from "./HomePage";

beforeEach(() => window.history.replaceState(null, "", "/"));

describe("HomePage", () => {
  it("points at the control center", () => {
    renderWithHints(<HomePage />);
    expect(screen.getByRole("heading", { name: "conversations come next" })).toBeInTheDocument();
    expect(screen.getByText("start agents from the control center for now")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: "open control center" }));
    expect(window.location.pathname).toBe("/townhall");
    expect(shownHints()).toBe("/");
  });
});
