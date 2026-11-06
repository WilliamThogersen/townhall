import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProfileSection } from "./ProfileSection";

const HELD = "townhall.db is held by another townhall";

interface Call {
  readonly method: string;
  readonly url: string;
  readonly body: unknown;
}

function reply(status: number, data: unknown, error: string | null): Response {
  return Response.json({ success: status < 400, data, error }, { status });
}

function fakeDaemon(initial = "") {
  let profile = initial;
  let calls: readonly Call[] = [];
  const failing = new Set<string>();
  const fetch = vi.fn(async (url: string, init?: RequestInit) => {
    const method = init?.method ?? "GET";
    const body = typeof init?.body === "string" ? (JSON.parse(init.body) as { text: string }) : undefined;
    calls = [...calls, { method, url, body }];
    if (url !== "/api/profile") return reply(404, null, "not found");
    if (failing.has(method)) return reply(503, null, HELD);
    if (method === "PUT") profile = body?.text ?? "";
    if (method === "DELETE") profile = "";
    return reply(200, { text: profile }, null);
  });
  return {
    fetch,
    failing,
    profile: () => profile,
    calls: (method: string) => calls.filter((call) => call.method === method),
  };
}

let daemon = fakeDaemon();

function setup(initial = "", learnAboutMe = false) {
  daemon = fakeDaemon(initial);
  vi.stubGlobal("fetch", daemon.fetch);
  return render(<ProfileSection learnAboutMe={learnAboutMe} />);
}

async function loaded(text: string) {
  const box = await screen.findByRole("textbox", { name: "Profile" });
  await waitFor(() => expect(box).toHaveValue(text));
  return box;
}

const save = () => screen.getByRole("button", { name: "save" });

afterEach(() => vi.unstubAllGlobals());

describe("ProfileSection", () => {
  it("shows the empty state, an empty box, save disabled and no reset", async () => {
    setup();
    const box = await screen.findByRole("textbox", { name: "Profile" });
    expect(await screen.findByText(/^no profile yet: write how you work/)).toBeVisible();
    expect(box).toHaveValue("");
    expect(save()).toBeDisabled();
    expect(screen.queryByRole("button", { name: "reset" })).not.toBeInTheDocument();
  });

  it("shows the saved text with reset available", async () => {
    setup("terse, tests first");
    await loaded("terse, tests first");
    expect(screen.queryByText(/no profile yet/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "reset" })).toBeEnabled();
    expect(save()).toBeDisabled();
  });

  it("edits and saves, then settles", async () => {
    setup("old");
    const box = await loaded("old");
    await userEvent.clear(box);
    await userEvent.type(box, "new text");
    expect(screen.getByText("unsaved changes")).toBeVisible();
    expect(save()).toBeEnabled();
    await userEvent.click(save());
    await waitFor(() => expect(save()).toBeDisabled());
    expect(daemon.calls("PUT")).toEqual([{ method: "PUT", url: "/api/profile", body: { text: "new text" } }]);
    expect(screen.queryByText("unsaved changes")).not.toBeInTheDocument();
    expect(box).toHaveValue("new text");
  });

  it("saves a first profile and drops the empty state", async () => {
    setup();
    const box = await loaded("");
    await userEvent.type(box, "hello");
    await userEvent.click(save());
    await waitFor(() => expect(screen.queryByText(/no profile yet/)).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "reset" })).toBeInTheDocument();
  });

  it("keeps the draft and says why when saving fails, until dismissed", async () => {
    setup("old");
    const box = await loaded("old");
    daemon.failing.add("PUT");
    await userEvent.type(box, " more");
    await userEvent.click(save());
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(`error: profile not saved: ${HELD}`);
    expect(box).toHaveValue("old more");
    expect(save()).toBeEnabled();
    await userEvent.click(within(alert).getByRole("button", { name: "dismiss" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows a daemon 400 as a save error", async () => {
    setup("old");
    const box = await loaded("old");
    daemon.fetch.mockImplementationOnce(async () => reply(400, null, "profile is too long"));
    await userEvent.type(box, "x");
    await userEvent.click(save());
    expect(await screen.findByRole("alert")).toHaveTextContent("error: profile not saved: profile is too long");
  });

  it("asks before resetting, and cancel sends nothing", async () => {
    setup("keep me");
    await loaded("keep me");
    await userEvent.click(screen.getByRole("button", { name: "reset" }));
    const confirm = screen.getByRole("group", { name: "reset profile" });
    expect(confirm).toHaveTextContent("reset your profile? the saved text is deleted for good");
    const cancel = within(confirm).getByRole("button", { name: "cancel" });
    expect(cancel).toHaveFocus();
    await userEvent.click(cancel);
    expect(screen.queryByRole("group", { name: "reset profile" })).not.toBeInTheDocument();
    expect(daemon.calls("DELETE")).toEqual([]);
    expect(screen.getByRole("textbox", { name: "Profile" })).toHaveValue("keep me");
  });

  it("deletes the profile once confirmed and shows the empty state", async () => {
    setup("keep me");
    await loaded("keep me");
    await userEvent.click(screen.getByRole("button", { name: "reset" }));
    await userEvent.click(within(screen.getByRole("group", { name: "reset profile" })).getByRole("button", { name: "reset" }));
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Profile" })).toHaveValue(""));
    expect(daemon.calls("DELETE")).toHaveLength(1);
    expect(daemon.profile()).toBe("");
    expect(screen.getByText(/^no profile yet/)).toBeVisible();
    expect(screen.queryByRole("group", { name: "reset profile" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "reset" })).not.toBeInTheDocument();
  });

  it("cancels the confirm with escape", async () => {
    setup("keep me");
    await loaded("keep me");
    await userEvent.click(screen.getByRole("button", { name: "reset" }));
    expect(screen.getByRole("group", { name: "reset profile" })).toBeVisible();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("group", { name: "reset profile" })).not.toBeInTheDocument();
    expect(daemon.calls("DELETE")).toEqual([]);
  });

  it("keeps the text and says why when reset fails", async () => {
    setup("keep me");
    await loaded("keep me");
    daemon.failing.add("DELETE");
    await userEvent.click(screen.getByRole("button", { name: "reset" }));
    await userEvent.click(within(screen.getByRole("group", { name: "reset profile" })).getByRole("button", { name: "reset" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(`error: profile not reset: ${HELD}`);
    expect(screen.getByRole("textbox", { name: "Profile" })).toHaveValue("keep me");
    expect(screen.queryByRole("group", { name: "reset profile" })).not.toBeInTheDocument();
  });

  it("says when the profile cannot be loaded, locks the box and retries", async () => {
    daemon = fakeDaemon("from daemon");
    daemon.failing.add("GET");
    vi.stubGlobal("fetch", daemon.fetch);
    render(<ProfileSection learnAboutMe={false} />);
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(`error: profile not loaded: ${HELD}`);
    expect(screen.getByRole("textbox", { name: "Profile" })).toBeDisabled();
    expect(save()).toBeDisabled();
    expect(screen.queryByText(/no profile yet/)).not.toBeInTheDocument();
    daemon.failing.delete("GET");
    await userEvent.click(within(alert).getByRole("button", { name: "retry" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(screen.getByRole("textbox", { name: "Profile" })).toHaveValue("from daemon");
    expect(screen.getByRole("textbox", { name: "Profile" })).toBeEnabled();
  });

  it("says it is not used while learn about me is off, yet stays editable", async () => {
    setup("", false);
    const box = await loaded("");
    expect(screen.getByText("not used while learn about me is off.")).toBeVisible();
    expect(screen.queryByText("townhall reads this in its own conversations.")).not.toBeInTheDocument();
    await userEvent.type(box, "x");
    expect(box).toHaveValue("x");
    expect(save()).toBeEnabled();
  });

  it("says townhall reads it while learn about me is on", async () => {
    setup("", true);
    await loaded("");
    expect(screen.getByText("townhall reads this in its own conversations.")).toBeVisible();
    expect(screen.queryByText("not used while learn about me is off.")).not.toBeInTheDocument();
  });

  it("limits the box to 20 000 characters", async () => {
    setup();
    expect(await screen.findByRole("textbox", { name: "Profile" })).toHaveAttribute("maxlength", "20000");
  });
});
