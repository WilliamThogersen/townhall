import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Banner } from "./Banner";

describe("Banner", () => {
  it("renders an error as an alert with the base class only", () => {
    render(<Banner level="error">cannot reach the daemon: connection refused</Banner>);
    const banner = screen.getByRole("alert");
    expect(banner).toHaveClass("th-banner");
    expect(banner).not.toHaveClass("th-banner--warn");
    expect(banner).not.toHaveClass("th-banner--info");
    expect(banner).toHaveTextContent("error: cannot reach the daemon: connection refused");
  });

  it("renders warn as a status with the warn class", () => {
    render(<Banner level="warn">shop has no base branch called develop</Banner>);
    const banner = screen.getByRole("status");
    expect(banner).toHaveClass("th-banner", "th-banner--warn");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("renders info as a status with the info class", () => {
    render(<Banner level="info">conversation continued from older</Banner>);
    expect(screen.getByRole("status")).toHaveClass("th-banner", "th-banner--info");
  });

  it("starts with the level prefix in a th-level span", () => {
    render(<Banner level="warn">careful</Banner>);
    const level = screen.getByText("warn:");
    expect(level.tagName).toBe("SPAN");
    expect(level).toHaveClass("th-level");
    expect(level.parentElement?.firstElementChild).toBe(level);
  });

  it("renders the message in its own span", () => {
    render(<Banner level="info">conversation continued</Banner>);
    const message = screen.getByText("conversation continued");
    expect(message.tagName).toBe("SPAN");
    expect(message).not.toHaveClass("th-level");
  });

  it("renders the action last", () => {
    render(
      <Banner level="error" action={<button className="th-btn th-btn--sm">retry</button>}>
        cannot reach the daemon
      </Banner>,
    );
    const action = screen.getByRole("button", { name: "retry" });
    expect(screen.getByRole("alert").lastElementChild).toBe(action);
  });

  it("renders no button without an action", () => {
    render(<Banner level="info">done</Banner>);
    expect(screen.queryByRole("button")).toBeNull();
  });
});
