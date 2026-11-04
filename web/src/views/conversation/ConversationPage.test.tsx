import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as client from "../../api/client";
import * as conversationsApi from "../../api/conversations";
import { ApiError } from "../../api/errors";
import { aConversation, aDetail, anEntry, aRepo, aTask, aYouEntry } from "../../api/fixtures";
import * as reposApi from "../../api/repos";
import type { ConversationDetail } from "../../api/types";
import { TestDataProvider } from "../../data/testing";
import { DEFAULT_SETTINGS, type Settings } from "../../settings/settings";
import { createSettingsStore } from "../../settings/settingsStore";
import { SettingsStoreContext } from "../../settings/useSettings";
import { KeyHintsProvider, useCurrentKeyHints } from "../../shell/keyHints";
import { ConversationPage } from "./ConversationPage";

vi.mock("./session", () => ({
  SessionPanel: (props: { detail: ConversationDetail; openTask: string | null; onOpenTask: (n: string) => void }) => (
    <aside className="th-session" aria-label="Session">
      <span>open: {props.openTask ?? "none"}</span>
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
  const wrap = (children: ReactNode) => (
    <SettingsStoreContext value={store}>
      <TestDataProvider>
        <KeyHintsProvider>
          {children}
          <Hints />
        </KeyHintsProvider>
      </TestDataProvider>
    </SettingsStoreContext>
  );
  render(wrap(<ConversationPage id={id} />));
  return { put };
}

const detail = aDetail({
  conversation: aConversation({ title: "login redirect loop", linkedRepo: "townhall" }),
  entries: [
    aYouEntry(1, "the login loops"),
    anEntry(2, { kind: "tasksCreated", tasks: ["fix-login"] }),
  ],
  tasks: [aTask({ name: "fix-login", repo: "townhall", state: "working" })],
});

function layout(): HTMLElement {
  const found = document.querySelector<HTMLElement>(".th-convo-layout");
  if (found === null) throw new Error("no conversation layout");
  return found;
}

beforeEach(() => {
  window.history.replaceState(null, "", "/c/c1");
  vi.spyOn(reposApi, "listRepos").mockResolvedValue([aRepo({ name: "townhall", baseBranch: "main" })]);
});

afterEach(() => vi.restoreAllMocks());

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
    await userEvent.click(await screen.findByRole("button", { name: "stop" }));
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
