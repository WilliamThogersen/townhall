import { describe, expect, it } from "vitest";
import { paneStreamView } from "./paneStream";

const stream = (status: "connecting" | "open" | "reconnecting" | "closed", error: string | null = null) => ({
  status,
  error,
});

describe("paneStreamView", () => {
  it("shows live while the socket is open", () => {
    expect(paneStreamView(stream("open"), true)).toEqual({ status: "open", glyph: "●", label: "live" });
  });

  it("shows connecting with no glyph before the first open", () => {
    expect(paneStreamView(stream("connecting"), true)).toEqual({
      status: "connecting",
      glyph: null,
      label: "connecting",
    });
  });

  it("keeps the connecting look while it reconnects", () => {
    expect(paneStreamView(stream("reconnecting"), true)).toEqual({
      status: "connecting",
      glyph: null,
      label: "reconnecting",
    });
  });

  it("reads a missing session as ended", () => {
    expect(paneStreamView(stream("closed", "agent `fix-login` not found"), true)).toEqual({
      status: "ended",
      glyph: "○",
      label: "ended: session closed",
    });
  });

  it("shows any other server error as an error with its reason", () => {
    expect(paneStreamView(stream("closed", "`tmux capture-pane` exited with 1"), true)).toEqual({
      status: "error",
      glyph: "×",
      label: "error: `tmux capture-pane` exited with 1",
    });
  });

  it("shows ended for a dead agent whatever the socket says", () => {
    expect(paneStreamView(stream("open"), false).status).toBe("ended");
    expect(paneStreamView(stream("connecting"), false).label).toBe("ended: session closed");
  });

  it("shows ended for a closed socket without a reason", () => {
    expect(paneStreamView(stream("closed"), true).status).toBe("ended");
  });
});
