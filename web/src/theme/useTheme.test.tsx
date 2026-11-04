import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { stubColorScheme } from "./testMatchMedia";

async function renderProbe() {
  vi.resetModules();
  const theme = await import("./theme");
  const { useTheme } = await import("./useTheme");
  function Probe() {
    const { choice, resolved, setChoice } = useTheme();
    return (
      <div>
        <p>choice: {choice}</p>
        <p>resolved: {resolved}</p>
        <button type="button" onClick={() => setChoice("light")}>
          light
        </button>
      </div>
    );
  }
  render(<Probe />);
  return theme;
}

beforeEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("useTheme", () => {
  it("renders the current choice", async () => {
    localStorage.setItem("th-theme", "dark");
    await renderProbe();
    expect(screen.getByText("choice: dark")).toBeInTheDocument();
    expect(screen.getByText("resolved: dark")).toBeInTheDocument();
  });

  it("re-renders and applies the theme after setChoice", async () => {
    await renderProbe();
    await userEvent.click(screen.getByRole("button", { name: "light" }));
    expect(screen.getByText("choice: light")).toBeInTheDocument();
    expect(screen.getByText("resolved: light")).toBeInTheDocument();
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
  });

  it("re-renders when the os changes while the choice is system", async () => {
    const os = stubColorScheme(false);
    localStorage.setItem("th-theme", "system");
    const theme = await renderProbe();
    act(() => theme.initTheme());
    expect(screen.getByText("resolved: dark")).toBeInTheDocument();
    act(() => os.setLight(true));
    expect(screen.getByText("choice: system")).toBeInTheDocument();
    expect(screen.getByText("resolved: light")).toBeInTheDocument();
  });
});
