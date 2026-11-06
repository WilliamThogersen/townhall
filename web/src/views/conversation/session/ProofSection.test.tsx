import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as artifactsApi from "../../../api/artifacts";
import { anArtifact } from "../../../api/fixtures";
import type { Artifact } from "../../../api/types";
import { ProofSection } from "./ProofSection";

afterEach(() => vi.restoreAllMocks());

const proof: readonly Artifact[] = [
  anArtifact({ id: "s1", name: "after.png", kind: "screenshot", mime: "image/png", authorRole: "tester", caption: "the redirect now lands on /home" }),
  anArtifact({ id: "n1", name: "notes.md", kind: "note", mime: "text/markdown; charset=utf-8", authorRole: "coder", caption: "what I changed" }),
  anArtifact({ id: "l1", name: "run.log", kind: "log", authorRole: "tester", size: 2048 }),
  anArtifact({ id: "r1", name: "review.md", kind: "report", authorRole: "reviewer" }),
];

function show(artifacts: readonly Artifact[] = proof) {
  vi.spyOn(artifactsApi, "listArtifacts").mockResolvedValue(artifacts);
  return render(<ProofSection task="fix-login" />);
}

const group = (role: string) => screen.findByRole("region", { name: `${role} proof` });

describe("ProofSection", () => {
  it("counts the proof and groups it by author role", async () => {
    show();
    expect(await screen.findByRole("heading", { name: "proof 4" })).toBeInTheDocument();
    const roles = (await screen.findAllByRole("region")).map((r) => r.getAttribute("aria-label"));
    expect(roles).toEqual(["coder proof", "tester proof", "reviewer proof"]);
    const tester = await group("tester");
    expect(within(tester).getByText("after.png")).toBeInTheDocument();
    expect(within(tester).getByText("run.log")).toBeInTheDocument();
    expect(within(await group("coder")).getByText("notes.md")).toBeInTheDocument();
  });

  it("shows a screenshot as a thumbnail with its caption", async () => {
    show();
    const tester = await group("tester");
    const image = within(tester).getByRole("img", { name: "the redirect now lands on /home" });
    expect(image).toHaveAttribute("src", "/api/tasks/fix-login/artifacts/s1");
    expect(within(tester).getByText("the redirect now lands on /home")).toBeInTheDocument();
  });

  it("falls back to the file name as the alt text without a caption", async () => {
    show([anArtifact({ id: "s2", name: "plain.png", kind: "screenshot", mime: "image/png" })]);
    expect(await screen.findByRole("img", { name: "plain.png" })).toBeInTheDocument();
  });

  it("opens the lightbox from a thumbnail", async () => {
    show();
    await userEvent.click(await screen.findByRole("button", { name: "open after.png" }));
    expect(document.querySelector(".th-lightbox")).not.toBeNull();
  });

  it("reads a log only when it is opened, as text", async () => {
    show();
    const read = vi.spyOn(artifactsApi, "readArtifactText").mockResolvedValue("<b>3 passed</b>\n0 failed");
    const tester = await group("tester");
    expect(read).not.toHaveBeenCalled();
    const toggle = within(tester).getByRole("button", { name: "show run.log" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(toggle);
    const text = await within(tester).findByLabelText("run.log contents");
    expect(text.tagName).toBe("PRE");
    expect(text.textContent).toBe("<b>3 passed</b>\n0 failed");
    expect(text.querySelector("b")).toBeNull();
    expect(read).toHaveBeenCalledWith("fix-login", "l1", expect.any(AbortSignal));
    expect(within(tester).getByRole("button", { name: "hide run.log" })).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(within(tester).getByRole("button", { name: "hide run.log" }));
    expect(within(tester).queryByLabelText("run.log contents")).toBeNull();
  });

  it("says when a log cannot be read", async () => {
    show();
    vi.spyOn(artifactsApi, "readArtifactText").mockRejectedValue(new Error("gone"));
    await userEvent.click(await screen.findByRole("button", { name: "show notes.md" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("gone");
  });

  it("says no proof yet without artifacts", async () => {
    show([]);
    expect(await screen.findByRole("heading", { name: "proof 0" })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText("no proof yet")).toBeInTheDocument());
    expect(screen.queryAllByRole("region")).toEqual([]);
  });

  it("says when the proof cannot be loaded", async () => {
    vi.spyOn(artifactsApi, "listArtifacts").mockRejectedValue(new Error("daemon down"));
    render(<ProofSection task="fix-login" />);
    expect(await screen.findByText(/cannot load proof/)).toHaveTextContent("daemon down");
  });
});
