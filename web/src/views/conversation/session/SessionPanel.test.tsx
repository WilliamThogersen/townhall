import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from "vitest";
import * as artifactsApi from "../../../api/artifacts";
import { aConversation, aDetail, anArtifact, anAttachment, anEntry, aRun, aRunStep, aTask } from "../../../api/fixtures";
import * as runsApi from "../../../api/runs";
import type { ConversationDetail } from "../../../api/types";
import { renderWithHints } from "../../../test/renderWithHints";
import { SessionPanel } from "./index";

const tasks = [
  aTask({ name: "session-ttl", title: "read the session ttl from config", state: "merged", pr: { number: 38, url: "https://git.test/38", checks: "passed" } }),
  aTask({ name: "auth-tests", title: "cover expiry, logout and the redirect target", state: "working" }),
  aTask({ name: "logout-cookie", title: "clear the cookie on logout", state: "queued", after: ["fix-login"] }),
  aTask({ name: "redirect-guard", title: "stop /login redirecting signed-in users", state: "inCi", pr: { number: 41, url: "https://git.test/41", checks: "failed" } }),
  aTask({ name: "fix-login", title: "clear the session before redirecting", state: "needsYou" }),
];

const fullDetail = (over: Partial<ConversationDetail> = {}): ConversationDetail =>
  aDetail({
    conversation: aConversation({ id: "login-loop", context: { used: 56_000, limit: 200_000 } }),
    entries: [anEntry(2, { kind: "repo", repo: "townhall", baseBranch: "main", rule: "named", reason: "you named it" })],
    tasks,
    attachments: [
      anAttachment({ id: "i1", name: "redirect-loop.png" }),
      anAttachment({ id: "i2", name: "network.png" }),
      anAttachment({ id: "f1", name: "localhost.har", mime: "application/json", kind: "file", size: 49_152 }),
    ],
    ...over,
  });

let listArtifacts: MockInstance<typeof artifactsApi.listArtifacts>;
let listRuns: MockInstance<typeof runsApi.listRuns>;

beforeEach(() => {
  listRuns = vi.spyOn(runsApi, "listRuns").mockResolvedValue([]);
  listArtifacts = vi.spyOn(artifactsApi, "listArtifacts").mockResolvedValue([]);
});
afterEach(() => vi.restoreAllMocks());

function renderPanel(detail = fullDetail(), openTask: string | null = null) {
  const onOpenTask = vi.fn();
  const view = renderWithHints(<SessionPanel detail={detail} openTask={openTask} onOpenTask={onOpenTask} />);
  return { ...view, onOpenTask };
}

const panel = () => screen.getByRole("complementary", { name: "Session" });
const rows = () => [...panel().querySelectorAll<HTMLLIElement>(".th-task")];
const rowAt = (index: number): HTMLLIElement => {
  const found = rows()[index];
  if (!found) throw new Error(`no task row ${index}`);
  return found;
};
const rowButton = (name: string) => panel().querySelector<HTMLButtonElement>(`button[data-task="${name}"]`)!;
const lineAfter = (heading: RegExp) => screen.getByRole("heading", { name: heading }).nextElementSibling as HTMLElement;

describe("SessionPanel head", () => {
  it("names the panel, counts the tasks and keeps the full summary as the title", () => {
    renderPanel();
    const head = panel().querySelector(".th-session-head")!;
    expect(within(head as HTMLElement).getByRole("heading", { name: "session" })).toBeInTheDocument();
    const count = within(head as HTMLElement).getByText("5 tasks");
    expect(count).toHaveAttribute(
      "title",
      "5 tasks · 1 needs you · 1 working · 1 queued · 1 in ci · 1 merged",
    );
  });

  it("links to the control center filtered to this conversation", () => {
    renderPanel();
    expect(screen.getByRole("link", { name: "control center ↗" })).toHaveAttribute("href", "/townhall?c=login-loop");
  });

  it("shows why a held back task does not start", () => {
    const why = "`rev-1` (reviewer) reported REJECT";
    renderPanel(fullDetail({ tasks: [aTask({ name: "ship", state: "queued", blocked: why })] }));
    const row = rowAt(0);
    expect(within(row).getByText("blocked")).toHaveClass("th-task-state");
    expect(within(row).getByText(why)).toHaveClass("th-task-blocked");
    expect(within(rowAt(0)).queryByText("queued")).toBeNull();
  });

  it("says 1 task for one", () => {
    renderPanel(fullDetail({ tasks: [aTask()] }));
    expect(within(panel().querySelector(".th-session-head") as HTMLElement).getByText("1 task")).toBeInTheDocument();
  });
});

describe("SessionPanel loop strip", () => {
  const stagesShown = () =>
    [...screen.getByLabelText("Tasks per stage").querySelectorAll(".th-loop-stage:not(.th-loop-stage--you)")].map(
      (s) => s.textContent,
    );

  it("counts tasks per stage the way the control center does and lights the stages that have any", () => {
    renderPanel();
    const loop = screen.getByLabelText("Tasks per stage");
    const stages = [...loop.querySelectorAll(".th-loop-stage:not(.th-loop-stage--you)")];
    expect(stages.map((s) => s.textContent)).toEqual(["plan 1", "code 1", "test 0", "review 2", "ship 1"]);
    expect(stages.map((s) => s.getAttribute("data-active"))).toEqual(["true", "true", null, "true", "true"]);
    expect(loop.querySelectorAll(".th-loop-arrow")).toHaveLength(4);
  });

  it("puts a task waiting on you, on ci or on failed checks under review", () => {
    renderPanel(
      fullDetail({
        tasks: [
          aTask({ name: "a", state: "needsYou" }),
          aTask({ name: "b", state: "inReview" }),
          aTask({ name: "c", state: "error", pr: { number: 41, url: null, checks: "failed" } }),
          aTask({ name: "d", state: "error" }),
          aTask({ name: "e", state: "ended" }),
        ],
      }),
    );
    expect(stagesShown()).toEqual(["plan 0", "code 1", "test 0", "review 3", "ship 0"]);
  });

  it("keeps the fixed line while the conversation has no run", async () => {
    renderPanel();
    await waitFor(() => expect(listRuns).toHaveBeenCalledWith("login-loop", expect.anything()));
    expect(stagesShown()).toEqual(["plan 1", "code 1", "test 0", "review 2", "ship 1"]);
    expect(screen.queryByLabelText("Steps of the run")).toBeNull();
  });

  it("keeps the fixed line when the runs cannot be read", async () => {
    listRuns.mockRejectedValue(new Error("daemon down"));
    renderPanel();
    await waitFor(() => expect(listRuns).toHaveBeenCalled());
    expect(stagesShown()).toEqual(["plan 1", "code 1", "test 0", "review 2", "ship 1"]);
  });

  it("shows the newest run's own step ids, their states and skipped", async () => {
    listRuns.mockResolvedValue([
      aRun({
        conversationId: "login-loop",
        steps: [
          aRunStep({ id: "plan", state: "done", round: 1 }),
          aRunStep({ id: "build", state: "started", round: 1 }),
          aRunStep({ id: "test" }),
          aRunStep({ id: "review", state: "skipped", round: 1, note: "no changed file matches" }),
          aRunStep({ id: "land" }),
        ],
      }),
      aRun({ id: "older", steps: [aRunStep({ id: "old", state: "done", round: 1 })] }),
    ]);
    renderPanel();
    const loop = await screen.findByLabelText("Steps of the run");
    const steps = [...loop.querySelectorAll(".th-loop-stage:not(.th-loop-stage--you)")];
    expect(steps.map((s) => s.textContent)).toEqual(["plan ✓", "build 1", "test ·", "review skipped", "land ·"]);
    expect(steps.map((s) => s.getAttribute("data-active"))).toEqual([null, "true", null, null, null]);
    expect(loop.querySelectorAll(".th-loop-arrow")).toHaveLength(4);
    expect(screen.queryByLabelText("Tasks per stage")).toBeNull();
  });

  it("keeps the needs you badge beside the run's steps", async () => {
    listRuns.mockResolvedValue([aRun({ steps: [aRunStep({ id: "plan", state: "done", round: 1 })] })]);
    renderPanel();
    await screen.findByLabelText("Steps of the run");
    expect(panel().querySelector(".th-loop-stage--you")).toHaveTextContent("needs you 1");
  });

  it("puts needs you on its own line only while a task needs you", () => {
    const { unmount } = renderPanel();
    const you = panel().querySelector(".th-loop-stage--you")!;
    expect(you).toHaveTextContent("needs you 1");
    expect(you.querySelector(".th-status")).toHaveAttribute("data-state", "waiting");
    unmount();
    renderPanel(fullDetail({ tasks: [aTask({ state: "working" })] }));
    expect(panel().querySelector(".th-loop-stage--you")).toBeNull();
  });
});

describe("SessionPanel tasks", () => {
  it("sorts the rows by attention", () => {
    renderPanel();
    expect(screen.getByRole("heading", { name: "tasks 5" })).toBeInTheDocument();
    expect(rows().map((row) => row.querySelector(".th-task-name")?.textContent)).toEqual([
      "fix-login",
      "redirect-guard",
      "auth-tests",
      "logout-cookie",
      "session-ttl",
    ]);
  });

  it("shows the state of every pull request the daemon tracks for a task", () => {
    const tracked = (number: number, status: "green" | "conflict", detail: string | null) => ({
      repo: "shop",
      number,
      url: `https://git.test/${number}`,
      branch: "townhall/x",
      base: "main",
      owner: "x",
      pusher: null,
      status,
      detail,
      updatedAt: "2026-11-13T10:00:00.000Z",
    });
    const detail = aDetail({
      tasks: [aTask({ name: "x", state: "inReview", prs: [tracked(7, "green", null), tracked(9, "conflict", "a.rs")] })],
    });
    renderPanel(fullDetail({ tasks: detail.tasks }));
    const items = [...panel().querySelectorAll(".th-task-pr")];
    expect(items.map((i) => i.textContent)).toEqual(["#7 ready to merge", "#9 conflict: a.rs"]);
    expect(items[1]).toHaveAttribute("data-status", "conflict");
  });

  it("shows glyph, state words, title, tags and the pull request link", () => {
    renderPanel();
    const [needs, guard, working, queued, merged] = [rowAt(0), rowAt(1), rowAt(2), rowAt(3), rowAt(4)];
    expect(needs).toHaveAttribute("data-state", "waiting");
    expect(needs.querySelector(".th-status")).toHaveAttribute("data-state", "waiting");
    expect(needs.querySelector(".th-task-state")).toHaveTextContent("needs you");
    expect(needs.querySelector(".th-task-title")).toHaveTextContent("clear the session before redirecting");
    expect(guard).toHaveAttribute("data-state", "error");
    expect(guard.querySelector(".th-task-state")).toHaveTextContent("checks failed");
    expect(guard.querySelector(".th-tag:not(.th-tag--agent)")).toHaveTextContent("pr open");
    expect(within(guard).getByRole("link", { name: "#41" })).toHaveAttribute("href", "https://git.test/41");
    expect(working).not.toHaveAttribute("data-state");
    expect(working.querySelector(".th-status")).toHaveAttribute("data-state", "working");
    expect(working.querySelector(".th-status .th-spin")).not.toBeNull();
    expect(queued.querySelector(".th-task-state")).toHaveTextContent("after fix-login");
    expect(queued.querySelector(".th-status")).toHaveAttribute("data-state", "queued");
    expect(merged).toHaveAttribute("data-state", "merged");
    expect(merged.querySelector(".th-tag:not(.th-tag--agent)")).toBeNull();
    expect(merged.querySelector(".th-tag--agent")).toHaveTextContent("builder");
    expect(within(merged).getByRole("link", { name: "#38" })).toBeInTheDocument();
  });

  it("shows a pull request without a url as plain text", () => {
    renderPanel(fullDetail({ tasks: [aTask({ state: "inReview", pr: { number: 7, url: null, checks: "none" } })] }));
    const link = rowAt(0).querySelector(".th-task-link")!;
    expect(link.tagName).toBe("SPAN");
    expect(link).toHaveTextContent("#7");
  });

  it("marks the open task, ignoring case", () => {
    renderPanel(fullDetail(), "Auth-Tests");
    expect(rowButton("auth-tests")).toHaveAttribute("aria-pressed", "true");
    expect(rowButton("fix-login")).toHaveAttribute("aria-pressed", "false");
  });

  it("opens a task when its row is clicked", async () => {
    const { onOpenTask } = renderPanel();
    await userEvent.click(rowButton("logout-cookie"));
    expect(onOpenTask).toHaveBeenCalledWith("logout-cookie");
  });

  it("says no tasks yet without tasks", () => {
    renderPanel(fullDetail({ tasks: [] }));
    expect(screen.getByRole("heading", { name: "tasks 0" })).toBeInTheDocument();
    expect(lineAfter(/^tasks/)).toHaveTextContent("no tasks yet");
  });
});

describe("SessionPanel proof", () => {
  it("shows no proof section while no task is open", () => {
    renderPanel();
    expect(screen.queryByRole("heading", { name: /^proof/ })).toBeNull();
    expect(listArtifacts).not.toHaveBeenCalled();
  });

  it("shows the proof of the open task, matched ignoring case", async () => {
    listArtifacts.mockResolvedValue([anArtifact({ task: "auth-tests", name: "run.log", authorRole: "tester" })]);
    renderPanel(fullDetail(), "Auth-Tests");
    expect(await screen.findByRole("heading", { name: "proof 1" })).toBeInTheDocument();
    expect(listArtifacts.mock.calls[0]?.[0]).toBe("auth-tests");
    expect(await screen.findByRole("region", { name: "tester proof" })).toBeInTheDocument();
  });

  it("shows nothing for a task this conversation does not have", () => {
    renderPanel(fullDetail(), "elsewhere");
    expect(screen.queryByRole("heading", { name: /^proof/ })).toBeNull();
    expect(listArtifacts).not.toHaveBeenCalled();
  });

  it("follows the open task", async () => {
    const view = renderPanel(fullDetail(), "auth-tests");
    await screen.findByRole("heading", { name: "proof 0" });
    view.rerender(<SessionPanel detail={fullDetail()} openTask="fix-login" onOpenTask={vi.fn()} />);
    await waitFor(() => expect(listArtifacts.mock.calls.map((call) => call[0])).toContain("fix-login"));
  });
});

describe("SessionPanel repo, files and context", () => {
  it("shows the repo and why townhall picked it", () => {
    renderPanel();
    const line = lineAfter(/^repo$/);
    expect(line.children[0]).toHaveTextContent("~ townhall · main");
    expect(line.children[1]).toHaveTextContent("you named it");
  });

  it("marks the repo line so a long reason wraps under the repo", () => {
    renderPanel();
    expect(lineAfter(/^repo$/)).toHaveAttribute("data-line", "repo");
  });

  it("says no repo yet before townhall picked one", () => {
    renderPanel(fullDetail({ entries: [] }));
    expect(lineAfter(/^repo$/)).toHaveTextContent("no repo yet");
  });

  it("shows every image as a thumbnail and every file as a chip", () => {
    renderPanel();
    expect(screen.getByRole("heading", { name: "files 3" })).toBeInTheDocument();
    const files = panel().querySelector(".th-session-files")!;
    expect(files.querySelectorAll(".th-msg-images .th-thumb")).toHaveLength(2);
    expect(files.querySelector(".th-file-name")).toHaveTextContent("localhost.har");
  });

  it("keeps all images when there are more than three", () => {
    const images = ["a", "b", "c", "d", "e"].map((id) => anAttachment({ id, name: `${id}.png` }));
    renderPanel(fullDetail({ attachments: images }));
    expect(panel().querySelectorAll(".th-session-files .th-thumb")).toHaveLength(5);
  });

  it("opens the lightbox from a thumbnail", async () => {
    renderPanel();
    const thumbs = panel().querySelectorAll<HTMLButtonElement>(".th-session-files .th-thumb");
    await userEvent.click(thumbs.item(1));
    expect(document.querySelector(".th-lightbox")).not.toBeNull();
  });

  it("says no files yet without attachments", () => {
    renderPanel(fullDetail({ attachments: [] }));
    expect(lineAfter(/^files/)).toHaveTextContent("no files yet");
  });

  it("shows the context meter and the tokens", () => {
    renderPanel();
    const meter = panel().querySelector(".th-ctx")!;
    expect(meter).toHaveTextContent("28%");
    expect(meter).not.toHaveAttribute("data-level");
    expect(meter.querySelector<HTMLElement>(".th-ctx-bar")?.style.getPropertyValue("--used")).toBe("28%");
    expect(lineAfter(/^context$/)).toHaveTextContent("56k of 200k tokens");
  });

  it("turns the meter amber past 80 percent", () => {
    renderPanel(fullDetail({ conversation: aConversation({ context: { used: 172_000, limit: 200_000 } }) }));
    expect(panel().querySelector(".th-ctx")).toHaveAttribute("data-level", "high");
  });

  it("says no context yet before the first turn", () => {
    renderPanel(fullDetail({ conversation: aConversation({ context: null }) }));
    expect(panel().querySelector(".th-ctx")).toBeNull();
    expect(lineAfter(/^context$/)).toHaveTextContent("no context yet");
  });
});
