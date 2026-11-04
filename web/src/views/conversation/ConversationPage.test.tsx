import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "../../api/client";
import * as conversationsApi from "../../api/conversations";
import { ApiError } from "../../api/errors";
import { aConversation, aDetail, anAttachment, anEntry, aRepo, aTask, aYouEntry } from "../../api/fixtures";
import * as reposApi from "../../api/repos";
import type { ConversationDetail } from "../../api/types";
import { createFakeEvents, TestDataProvider } from "../../data/testing";
import { DEFAULT_SETTINGS, type Settings } from "../../settings/settings";
import { createSettingsStore } from "../../settings/settingsStore";
import { SettingsStoreContext } from "../../settings/useSettings";
import { KeyHintsProvider, useCurrentKeyHints } from "../../shell/keyHints";
import { ConversationPage } from "./ConversationPage";

vi.mock("./session", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./session")>()),
  SessionPanel: (props: { detail: ConversationDetail; openTask: string | null; onOpenTask: (n: string) => void }) => (
    <aside className="th-session" aria-label="Session">
      <span>open: {props.openTask ?? "none"}</span>
      <span>files {props.detail.attachments.length}</span>
      <button type="button" onClick={() => props.onOpenTask("fix-login")}>
        panel open fix-login
      </button>
    </aside>
  ),
  TaskTerminal: (props: { task: { name: string }; onClose: () => void }) => (
    <div className="th-panes">
      <span>terminal {props.task.name}</span>
      <button type="button" onClick={props.onClose}>
        close terminal
      </button>
    </div>
  ),
}));

function Hints() {
  return <output data-testid="hints">{useCurrentKeyHints().map((h) => `${h.keys}=${h.label}`).join(" ")}</output>;
}

function settingsStore(initial: Partial<Settings> = {}) {
  let saved: Settings = { ...DEFAULT_SETTINGS, ...initial };
  const put = vi.fn(async (key: keyof Settings, value: Settings[keyof Settings]) => {
    saved = { ...saved, [key]: value };
    return saved;
  });
  return { store: createSettingsStore({ load: async () => saved, put, storage: () => null }), put };
}

function renderPage(id = "c1", initial: Partial<Settings> = {}) {
  const { store, put } = settingsStore(initial);
  const events = createFakeEvents();
  const wrap = (children: ReactNode) => (
    <SettingsStoreContext value={store}>
      <TestDataProvider events={events}>
        <KeyHintsProvider>
          {children}
          <Hints />
        </KeyHintsProvider>
      </TestDataProvider>
    </SettingsStoreContext>
  );
  render(wrap(<ConversationPage id={id} />));
  return { put, events };
}

const detail = aDetail({
  conversation: aConversation({ title: "login redirect loop", linkedRepo: "townhall" }),
  entries: [
    aYouEntry(1, "the login loops"),
    anEntry(2, { kind: "tasksCreated", tasks: ["fix-login"] }),
  ],
  tasks: [aTask({ name: "fix-login", repo: "townhall", state: "working" })],
});

function headerOf(title: HTMLElement): HTMLElement {
  const found = title.closest<HTMLElement>(".th-convo-head");
  if (found === null) throw new Error("no conversation header");
  return found;
}

function layout(): HTMLElement {
  const found = document.querySelector<HTMLElement>(".th-convo-layout");
  if (found === null) throw new Error("no conversation layout");
  return found;
}

beforeEach(() => {
  window.history.replaceState(null, "", "/c/c1");
  vi.spyOn(reposApi, "listRepos").mockResolvedValue([aRepo({ name: "townhall", baseBranch: "main" })]);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("ConversationPage states", () => {
  it("opens the conversation with its header, thread and session panel", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    renderPage();
    expect(await screen.findByRole("heading", { name: "login redirect loop" })).toBeInTheDocument();
    expect(await screen.findByText("~ townhall · main")).toBeInTheDocument();
    expect(screen.getByRole("log", { name: "Thread" })).toHaveTextContent("the login loops");
    expect(screen.getByRole("complementary", { name: "Session" })).toHaveTextContent("open: none");
    expect(layout()).not.toHaveAttribute("data-pane");
    expect(layout()).not.toHaveAttribute("data-rail");
    expect(screen.getByTestId("hints")).toHaveTextContent(
      "r=reply s=session panel ⏎=send ⇧⏎=new line esc=leave input",
    );
  });

  it("says when the conversation does not exist", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockRejectedValue(
      new ApiError("http", "conversation `gone` not found", 404),
    );
    renderPage("gone");
    expect(await screen.findByText("error: conversation gone not found")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "new conversation" })).toHaveAttribute("href", "/");
    expect(screen.getByTestId("hints")).toHaveTextContent("n=new conversation /=search");
  });

  it("waits for the daemon and retries", async () => {
    const get = vi
      .spyOn(conversationsApi, "getConversation")
      .mockRejectedValueOnce(new ApiError("network", "cannot reach the daemon: refused"))
      .mockResolvedValue(detail);
    renderPage();
    expect(await screen.findByRole("heading", { name: "cannot reach the daemon" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "retry" }));
    expect(await screen.findByRole("heading", { name: "login redirect loop" })).toBeInTheDocument();
    expect(get).toHaveBeenCalledTimes(2);
  });

  it("shows any other failure", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockRejectedValue(
      new ApiError("protocol", "unexpected response shape: conversation.title"),
    );
    renderPage();
    expect(await screen.findByText("error: unexpected response shape: conversation.title")).toBeInTheDocument();
  });
});

describe("ConversationPage terminal and session panel", () => {
  it("opens a task's terminal from the thread and closes it", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "open fix-login" }));
    expect(window.location.search).toBe("?t=fix-login");
    expect(screen.getByText("terminal fix-login")).toBeInTheDocument();
    expect(layout()).toHaveAttribute("data-pane", "true");
    await userEvent.click(screen.getByRole("button", { name: "close terminal" }));
    expect(window.location.search).toBe("");
    expect(layout()).not.toHaveAttribute("data-pane");
  });

  it("keeps the session panel for a task of another conversation", async () => {
    window.history.replaceState(null, "", "/c/c1?t=elsewhere");
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    renderPage();
    expect(await screen.findByText("open: elsewhere")).toBeInTheDocument();
    expect(layout()).not.toHaveAttribute("data-pane");
  });

  it("folds the session panel with the header toggle and s, and remembers it", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    const { put } = renderPage();
    const toggle = await screen.findByRole("button", { name: "session" });
    await waitFor(() => expect(toggle).toHaveAttribute("aria-expanded", "true"));
    await userEvent.click(toggle);
    expect(layout()).toHaveAttribute("data-rail", "closed");
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await waitFor(() => expect(put).toHaveBeenCalledWith("sessionPanelOpen", false));
    await userEvent.keyboard("s");
    expect(layout()).not.toHaveAttribute("data-rail");
  });

  it("opens the session panel as an overlay on a narrow window", async () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn((media: string) => ({ media, matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() })),
    );
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    const { put } = renderPage();
    const toggle = await screen.findByRole("button", { name: "session" });
    expect(layout()).not.toHaveAttribute("data-rail");
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await userEvent.keyboard("s");
    expect(layout()).toHaveAttribute("data-rail", "open");
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(toggle);
    expect(layout()).not.toHaveAttribute("data-rail");
    expect(put).not.toHaveBeenCalled();
  });

  it("ignores s while typing", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    renderPage();
    await screen.findByRole("heading", { name: "login redirect loop" });
    const field = document.createElement("textarea");
    document.body.append(field);
    field.focus();
    await userEvent.keyboard("s");
    expect(layout()).not.toHaveAttribute("data-rail");
    field.remove();
  });

  it("brings the session panel back from an open terminal", async () => {
    window.history.replaceState(null, "", "/c/c1?t=fix-login");
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    renderPage("c1", { sessionPanelOpen: false });
    expect(await screen.findByText("terminal fix-login")).toBeInTheDocument();
    expect(layout()).not.toHaveAttribute("data-rail");
    await userEvent.click(screen.getByRole("button", { name: "session" }));
    expect(window.location.search).toBe("");
    await waitFor(() => expect(layout()).not.toHaveAttribute("data-rail"));
    expect(screen.getByRole("complementary", { name: "Session" })).toBeInTheDocument();
  });
});

describe("ConversationPage key hints", () => {
  const PAGE_HINTS = "r=reply s=session panel ⏎=send ⇧⏎=new line esc=leave input";

  it("adds the panel's keys while it shows and the close key while a terminal is open", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    renderPage();
    const hints = screen.getByTestId("hints");
    await waitFor(() => expect(hints.textContent).toBe(`${PAGE_HINTS} j k=tasks ⏎=open terminal`));
    await userEvent.keyboard("s");
    await waitFor(() => expect(hints.textContent).toBe(PAGE_HINTS));
    await userEvent.click(screen.getByRole("button", { name: "open fix-login" }));
    await waitFor(() => expect(hints.textContent).toBe(`${PAGE_HINTS} esc=close terminal`));
  });

  it("leaves the panel's keys out without tasks", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue({ ...detail, tasks: [] });
    renderPage();
    await waitFor(() => expect(screen.getByRole("button", { name: "session" })).toHaveAttribute("aria-expanded", "true"));
    expect(screen.getByTestId("hints").textContent).toBe(PAGE_HINTS);
  });
});

describe("ConversationPage actions", () => {
  it("stops a running turn and shows a failure in a banner", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue({
      ...detail,
      conversation: { ...detail.conversation, status: "thinking" },
    });
    const stop = vi
      .spyOn(conversationsApi, "stopTurn")
      .mockRejectedValue(new ApiError("http", "conversation `c1` not found", 404));
    renderPage();
    const header = within(await screen.findByRole("heading", { name: "login redirect loop" }).then(headerOf));
    await userEvent.click(header.getByRole("button", { name: "stop" }));
    expect(stop).toHaveBeenCalledWith("c1");
    expect(await screen.findByRole("alert")).toHaveTextContent("error: conversation `c1` not found");
    await userEvent.click(screen.getByRole("button", { name: "dismiss" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("ends the conversation after the confirm", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    const end = vi.spyOn(conversationsApi, "endConversation").mockResolvedValue(aConversation({ ended: true }));
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "end" }));
    expect(screen.getByText("end login redirect loop? 1 task keeps running")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "end" }));
    expect(end).toHaveBeenCalledWith("c1");
  });

  it("sends a needs-you answer to the agent and a repo pick as a message", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue({
      ...detail,
      entries: [
        aYouEntry(1, "fix checkout"),
        anEntry(2, { kind: "repoAsk", question: "which one?", candidates: ["shop", "shop-b2b"], offerAll: false }),
        anEntry(3, { kind: "needsYou", agent: "fix-login", question: "commit? (y/n)", choices: ["y", "n"] }),
      ],
      tasks: [aTask({ name: "fix-login", state: "needsYou" })],
    });
    const send = vi.spyOn(client, "sendText").mockResolvedValue("fix-login");
    const message = vi.spyOn(conversationsApi, "sendMessage").mockResolvedValue(aYouEntry(4, "shop"));
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "send y" }));
    expect(send).toHaveBeenCalledWith("fix-login", "y");
    await userEvent.click(screen.getByRole("button", { name: "shop" }));
    expect(message).toHaveBeenCalledWith("c1", { text: "shop" });
  });
});

describe("ConversationPage composer and live thread", () => {
  it("sends from the composer and r focuses it", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    const send = vi.spyOn(conversationsApi, "sendMessage").mockResolvedValue(aYouEntry(3, "more tests"));
    renderPage();
    const box = await screen.findByRole("textbox", { name: "Message" });
    expect(box).not.toHaveFocus();
    await userEvent.keyboard("r");
    expect(box).toHaveFocus();
    await userEvent.type(box, "more tests{Enter}");
    expect(send).toHaveBeenCalledWith("c1", { text: "more tests" });
  });

  it("shows the drop zone while files are dragged over the conversation", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    renderPage();
    const view = (await screen.findByRole("heading", { name: "login redirect loop" })).closest("section");
    if (view === null) throw new Error("no conversation view");
    fireEvent.dragEnter(view, { dataTransfer: { types: ["Files"], files: [] } });
    expect(screen.getByText("drop to attach")).toHaveClass("th-drop");
    fireEvent.dragLeave(view, { dataTransfer: { types: ["Files"], files: [] } });
    expect(screen.queryByText("drop to attach")).toBeNull();
  });

  it("shows the images of your message", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue({
      ...detail,
      entries: [anEntry(1, { kind: "you", text: "look", attachments: ["a1"] })],
      attachments: [anAttachment({ id: "a1", name: "shot.png" })],
    });
    renderPage();
    const log = await screen.findByRole("log", { name: "Thread" });
    await waitFor(() => expect(log.querySelector(".th-msg-images .th-thumb")).not.toBeNull());
  });

  it("loads the files a new message names, so its thumbnails and the files count show at once", async () => {
    const shot = anAttachment({ id: "a9", name: "shot.png" });
    const notes = anAttachment({ id: "a10", name: "notes.txt", mime: "text/plain", kind: "file" });
    const sent = anEntry(3, { kind: "you", text: "look", attachments: ["a9", "a10"] });
    const get = vi
      .spyOn(conversationsApi, "getConversation")
      .mockResolvedValueOnce(detail)
      .mockResolvedValue({ ...detail, entries: [...detail.entries, sent], attachments: [shot, notes] });
    const { events } = renderPage();
    await screen.findByRole("heading", { name: "login redirect loop" });
    expect(screen.getByText("files 0")).toBeInTheDocument();
    act(() => events.emit({ type: "entryAppended", conversationId: "c1", entry: sent }));
    const log = screen.getByRole("log", { name: "Thread" });
    await waitFor(() => expect(log.querySelector(".th-msg-images .th-thumb")).not.toBeNull());
    expect(log.querySelector(".th-msg-files")).toHaveTextContent("notes.txt");
    expect(screen.getByText("files 2")).toBeInTheDocument();
    expect(get).toHaveBeenCalledTimes(2);
  });

  it("asks the daemon once for a file it does not list", async () => {
    const lost = anEntry(3, { kind: "you", text: "look", attachments: ["gone"] });
    const get = vi.spyOn(conversationsApi, "getConversation").mockResolvedValue({ ...detail, entries: [lost] });
    renderPage();
    await screen.findByRole("heading", { name: "login redirect loop" });
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 30));
    });
    expect(get).toHaveBeenCalledTimes(2);
  });

  it("adds entries and thinking as events arrive", async () => {
    vi.spyOn(conversationsApi, "getConversation").mockResolvedValue(detail);
    const { events } = renderPage();
    await screen.findByRole("heading", { name: "login redirect loop" });
    act(() => {
      events.emit({ type: "entryAppended", conversationId: "c1", entry: anEntry(3, { kind: "townhall", text: "on **it**" }) });
      events.emit({
        type: "conversationUpdated",
        conversation: { ...detail.conversation, status: "thinking", updatedAt: "2026-11-04T11:00:00.000Z" },
      });
    });
    const log = screen.getByRole("log", { name: "Thread" });
    expect(log.querySelector("strong")).toHaveTextContent("it");
    expect(within(log).getByRole("status")).toHaveTextContent("thinking");
    expect(screen.getByRole("textbox", { name: "Message" })).toBeDisabled();
  });
});
