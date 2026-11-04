import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWithHints, shownHints } from "../test/renderWithHints";
import { NotFound } from "./NotFound";

describe("NotFound", () => {
  it("names the path and goes home", () => {
    window.history.replaceState(null, "", "/nope");
    renderWithHints(<NotFound path="/nope" />);
    expect(screen.getByRole("heading", { name: "not found" })).toBeInTheDocument();
    expect(screen.getByText("nothing lives at /nope")).toBeInTheDocument();
    expect(shownHints()).toBe("n /");
    fireEvent.click(screen.getByRole("link", { name: "go home" }));
    expect(window.location.pathname).toBe("/");
  });
});
