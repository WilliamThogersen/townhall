import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWithHints, shownHints } from "../../test/renderWithHints";
import { SearchPage } from "./SearchPage";

describe("SearchPage", () => {
  it("says what comes next", () => {
    renderWithHints(<SearchPage />);
    expect(screen.getByRole("heading", { name: "search comes next" })).toBeInTheDocument();
    expect(screen.getByText("there are no conversations to search yet")).toBeInTheDocument();
    expect(shownHints()).toBe("n");
  });
});
