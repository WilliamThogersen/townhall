import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, isSettingValue, SETTING_KEYS, type Settings } from "../../settings/settings";
import { createSettingsStore } from "../../settings/settingsStore";
import { SettingsStoreContext } from "../../settings/useSettings";
import { KeyHintsProvider, useCurrentKeyHints } from "../../shell/keyHints";
import { saveThemeChoice } from "../../theme/theme";
import { SettingsPage } from "./SettingsPage";

const HELD = "townhall.db is held by another townhall";

interface Call {
  readonly method: string;
  readonly url: string;
  readonly body: unknown;
}

function reply(status: number, data: unknown, error: string | null): Response {
  return Response.json({ success: status < 400, data, error }, { status });
}

function fakeDaemon(initial: Partial<Settings> = {}) {
  let settings: Settings = { ...DEFAULT_SETTINGS, ...initial };
  let calls: readonly Call[] = [];
  let profile = "";
  const failing = new Set<string>();
  const answer = (method: string, url: string, body: { value?: unknown; text?: string } | undefined): Response => {
    if (url === "/api/profile") {
      if (failing.has("profile")) return reply(503, null, HELD);
      if (method === "PUT") profile = body?.text ?? "";
      if (method === "DELETE") profile = "";
      return reply(200, { text: profile }, null);
    }
    if (method === "GET" && url === "/api/settings") {
      return failing.has("GET") ? reply(503, null, HELD) : reply(200, settings, null);
    }
    const key = SETTING_KEYS.find((known) => url === `/api/settings/${known}`);
    if (method !== "PUT" || key === undefined) return reply(404, null, "not found");
    if (failing.has(key)) return reply(503, null, HELD);
    if (!isSettingValue(key, body?.value)) return reply(400, null, "invalid value");
    settings = { ...settings, [key]: body?.value };
    return reply(200, settings, null);
  };
  const fetch = vi.fn(async (url: string, init?: RequestInit) => {
    const method = init?.method ?? "GET";
    const body: unknown = typeof init?.body === "string" ? JSON.parse(init.body) : undefined;
    calls = [...calls, { method, url, body }];
    return answer(method, url, body as { value?: unknown; text?: string } | undefined);
  });
  return {
    fetch,
    failing,
    settings: () => settings,
    profile: () => profile,
    puts: () => calls.filter((call) => call.method === "PUT").map((call) => [call.url, call.body]),
  };
}

let daemon = fakeDaemon();

function renderPage(children = <SettingsPage />) {
  const store = createSettingsStore();
  return render(<SettingsStoreContext value={store}>{children}</SettingsStoreContext>);
}

async function renderLoaded() {
  const view = renderPage();
  await waitFor(() => expect(document.querySelector(".th-settings")).toHaveAttribute("aria-busy", "false"));
  return view;
}

function HintsProbe() {
  const hints = useCurrentKeyHints();
  return <p data-testid="hints">{hints.map((hint) => `${hint.keys}=${hint.label}`).join(" | ")}</p>;
}

const group = (name: string) => within(screen.getByRole("radiogroup", { name }));
function permissionHelp(): HTMLElement {
  const help = screen.getByText("permissions", { selector: ".th-setting-label" }).nextElementSibling;
  if (!(help instanceof HTMLElement)) throw new Error("permissions has no help line");
  return help;
}

function lastPut(): unknown {
  return daemon.puts().at(-1);
}

beforeEach(() => {
  saveThemeChoice("dark");
  localStorage.clear();
  daemon = fakeDaemon();
  vi.stubGlobal("fetch", daemon.fetch);
});

afterEach(() => vi.unstubAllGlobals());

describe("SettingsPage", () => {
  it("shows the four groups and how changes are saved", async () => {
    await renderLoaded();
    expect(screen.getByRole("heading", { level: 1, name: "settings" })).toBeInTheDocument();
    expect(screen.getByText("saved as you change them")).toBeInTheDocument();
    ["agents", "conversations", "appearance and notifications", "about you"].forEach((name) => {
      expect(screen.getByRole("heading", { level: 2, name })).toBeInTheDocument();
    });
    const saves = screen.getAllByRole("button", { name: /save/ });
    expect(saves).toHaveLength(1);
    expect(saves[0]?.closest(".th-setting-row")).toHaveTextContent("profile");
  });

  it("renders every row", async () => {
    await renderLoaded();
    const labels = [...document.querySelectorAll(".th-setting-label")].map((label) => label.textContent);
    expect(labels).toEqual([
      "permissions",
      "work without asking",
      "agents at once",
      "when the repo is unclear",
      "session panel",
      "theme",
      "notify me when",
      "learn about me",
      "profile",
    ]);
  });

  it("shows a profile problem in the profile row, not as the settings banner", async () => {
    daemon.failing.add("profile");
    await renderLoaded();
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(`error: profile not loaded: ${HELD}`);
    expect(alert.closest(".th-setting-row")).toHaveAttribute("data-wide", "true");
    expect(screen.getAllByRole("alert")).toHaveLength(1);
  });

  it("shows the profile row, editable, and its help follows the switch", async () => {
    await renderLoaded();
    expect(await screen.findByRole("textbox", { name: "Profile" })).toBeEnabled();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByText("not used while learn about me is off.")).toBeVisible();
    await userEvent.click(screen.getByRole("switch", { name: "Learn about me" }));
    expect(await screen.findByText("townhall reads this in its own conversations.")).toBeVisible();
  });

  it("switches learn about me, off by default, and explains what it keeps", async () => {
    await renderLoaded();
    const toggle = screen.getByRole("switch", { name: "Learn about me" });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    const help = screen.getByText("learn about me", { selector: ".th-setting-label" }).nextElementSibling;
    expect(help).toHaveTextContent(/how you work, your tone of voice, your typical flow/);
    expect(help).toHaveTextContent(/stored locally/);
    expect(help).toHaveTextContent(/edit or reset it below at any time/);
    await userEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-checked", "true");
    await waitFor(() => expect(daemon.settings().learnAboutMe).toBe(true));
    expect(lastPut()).toEqual(["/api/settings/learnAboutMe", { value: true }]);
  });

  it("rolls the learn about me switch back when saving fails", async () => {
    await renderLoaded();
    daemon.failing.add("learnAboutMe");
    const toggle = screen.getByRole("switch", { name: "Learn about me" });
    await userEvent.click(toggle);
    expect(await screen.findByRole("alert")).toHaveTextContent(`error: settings not saved: ${HELD}`);
    expect(toggle).toHaveAttribute("aria-checked", "false");
  });

  it("shows the keys that move through the settings", async () => {
    renderPage(
      <KeyHintsProvider>
        <SettingsPage />
        <HintsProbe />
      </KeyHintsProvider>,
    );
    expect(screen.getByTestId("hints")).toHaveTextContent("tab=next setting | ← →=change choice");
    await waitFor(() => expect(daemon.fetch).toHaveBeenCalled());
  });

  it("shows the defaults while the daemon answers", async () => {
    daemon = fakeDaemon({ permissionMode: "plan" });
    let answer: () => void = () => undefined;
    const held = new Promise<void>((resolve) => {
      answer = resolve;
    });
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string, init?: RequestInit) => {
        await held;
        return daemon.fetch(url, init);
      }),
    );
    renderPage();
    expect(document.querySelector(".th-settings")).toHaveAttribute("aria-busy", "true");
    expect(group("Permissions").getByRole("radio", { name: "accept edits" })).toBeChecked();
    answer();
    await waitFor(() => expect(group("Permissions").getByRole("radio", { name: "plan" })).toBeChecked());
    expect(document.querySelector(".th-settings")).toHaveAttribute("aria-busy", "false");
  });

  it("starts from the daemon's settings", async () => {
    daemon = fakeDaemon({ permissionMode: "plan", agentsAtOnce: 9, repoUnclear: "guess", notifyWhen: "failed" });
    vi.stubGlobal("fetch", daemon.fetch);
    await renderLoaded();
    expect(group("Permissions").getByRole("radio", { name: "plan" })).toBeChecked();
    expect(screen.getByRole("group", { name: "Agents at once" })).toHaveTextContent("9");
    expect(group("When the repo is unclear").getByRole("radio", { name: "guess" })).toBeChecked();
    expect(group("Notify me when").getByRole("radio", { name: "+ failed" })).toBeChecked();
  });

  it("changes the permission mode and its help line", async () => {
    await renderLoaded();
    expect(group("Permissions").getByRole("radio", { name: "accept edits" })).toBeChecked();
    expect(permissionHelp()).toHaveTextContent("edits go through, commands ask first.");
    const cases = [
      ["ask", "default", "agents ask before every edit or command."],
      ["plan", "plan", "agents plan first and change nothing."],
      ["auto", "auto", "agents decide what needs your approval."],
    ] as const;
    for (const [label, mode, help] of cases) {
      await userEvent.click(group("Permissions").getByRole("radio", { name: label }));
      expect(permissionHelp()).toHaveTextContent(help);
      expect(permissionHelp()).not.toHaveAttribute("data-tone");
      await waitFor(() => expect(daemon.settings().permissionMode).toBe(mode));
    }
    expect(daemon.puts()).toEqual([
      ["/api/settings/permissionMode", { value: "default" }],
      ["/api/settings/permissionMode", { value: "plan" }],
      ["/api/settings/permissionMode", { value: "auto" }],
    ]);
    expect(localStorage.getItem("th-settings")).toBeNull();
  });

  it("marks skip all as dangerous, saves it through the endpoint and explains it on demand", async () => {
    await renderLoaded();
    const skip = group("Permissions").getByRole("radio", { name: "skip all" });
    expect(skip.closest("label")).toHaveAttribute("data-tone", "danger");
    await userEvent.click(skip);
    await waitFor(() => expect(daemon.settings().permissionMode).toBe("bypassPermissions"));
    expect(lastPut()).toEqual(["/api/settings/permissionMode", { value: "bypassPermissions" }]);
    expect(permissionHelp()).toHaveAttribute("data-tone", "danger");
    expect(permissionHelp()).toHaveTextContent("agents never ask. only use on repos you trust.");
    const more = screen.getByRole("button", { name: "more" });
    expect(more).toHaveAttribute("aria-expanded", "false");
    const details = document.getElementById(more.getAttribute("aria-controls") ?? "");
    expect(details).not.toBeVisible();
    await userEvent.click(more);
    expect(details).toBeVisible();
    expect(details).toHaveTextContent("--dangerously-skip-permissions");
    expect(screen.getByRole("button", { name: "less" })).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(screen.getByRole("button", { name: "less" }));
    expect(details).not.toBeVisible();
  });

  it("drops the danger help when leaving skip all", async () => {
    await renderLoaded();
    await userEvent.click(group("Permissions").getByRole("radio", { name: "skip all" }));
    await userEvent.click(screen.getByRole("button", { name: "more" }));
    await userEvent.click(group("Permissions").getByRole("radio", { name: "ask" }));
    expect(screen.queryByRole("button", { name: /more|less/ })).not.toBeInTheDocument();
    expect(screen.queryByText(/--dangerously-skip-permissions/)).not.toBeInTheDocument();
    await userEvent.click(group("Permissions").getByRole("radio", { name: "skip all" }));
    expect(screen.getByRole("button", { name: "more" })).toHaveAttribute("aria-expanded", "false");
    await waitFor(() => expect(daemon.puts()).toHaveLength(3));
  });

  it("switches work without asking", async () => {
    await renderLoaded();
    const toggle = screen.getByRole("switch", { name: "Work without asking" });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await userEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-checked", "true");
    await waitFor(() => expect(daemon.settings().workWithoutAsking).toBe(true));
    expect(lastPut()).toEqual(["/api/settings/workWithoutAsking", { value: true }]);
  });

  it("steps agents at once", async () => {
    await renderLoaded();
    const stepper = within(screen.getByRole("group", { name: "Agents at once" }));
    await userEvent.click(stepper.getByRole("button", { name: "More" }));
    await userEvent.click(stepper.getByRole("button", { name: "Fewer" }));
    await userEvent.click(stepper.getByRole("button", { name: "Fewer" }));
    expect(stepper.getByText("5")).toBeInTheDocument();
    await waitFor(() => expect(daemon.settings().agentsAtOnce).toBe(5));
    expect(daemon.puts().map(([, body]) => body)).toEqual([{ value: 7 }, { value: 6 }, { value: 5 }]);
  });

  it("chooses what happens when the repo is unclear", async () => {
    await renderLoaded();
    expect(group("When the repo is unclear").getByRole("radio", { name: "ask" })).toBeChecked();
    await userEvent.click(group("When the repo is unclear").getByRole("radio", { name: "guess" }));
    await waitFor(() => expect(daemon.settings().repoUnclear).toBe("guess"));
  });

  it("switches the session panel default", async () => {
    await renderLoaded();
    const toggle = screen.getByRole("switch", { name: "Session panel open by default" });
    expect(toggle).toHaveAttribute("aria-checked", "true");
    await userEvent.click(toggle);
    await waitFor(() => expect(daemon.settings().sessionPanelOpen).toBe(false));
  });

  it("changes the theme of the app at once, without the daemon", async () => {
    await renderLoaded();
    expect(group("Theme").getByRole("radio", { name: "dark" })).toBeChecked();
    await userEvent.click(group("Theme").getByRole("radio", { name: "light" }));
    expect(document.documentElement).toHaveAttribute("data-theme", "light");
    expect(localStorage.getItem("th-theme")).toBe("light");
    expect(group("Theme").getByRole("radio", { name: "light" })).toBeChecked();
    expect(daemon.puts()).toEqual([]);
    expect(localStorage.getItem("th-settings")).toBeNull();
  });

  it("chooses when to notify", async () => {
    await renderLoaded();
    expect(group("Notify me when").getByRole("radio", { name: "needs you" })).toBeChecked();
    await userEvent.click(group("Notify me when").getByRole("radio", { name: "everything" }));
    await userEvent.click(group("Notify me when").getByRole("radio", { name: "+ failed" }));
    await waitFor(() => expect(daemon.settings().notifyWhen).toBe("failed"));
  });

  it("rolls a failed save back and says why until dismissed", async () => {
    await renderLoaded();
    daemon.failing.add("agentsAtOnce");
    const stepper = within(screen.getByRole("group", { name: "Agents at once" }));
    await userEvent.click(stepper.getByRole("button", { name: "More" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(`error: settings not saved: ${HELD}`);
    expect(alert.textContent).toMatch(/^error:/);
    expect(stepper.getByText("6")).toBeInTheDocument();
    expect(daemon.settings().agentsAtOnce).toBe(6);
    await userEvent.click(within(alert).getByRole("button", { name: "dismiss" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("says when the settings cannot be loaded and retries", async () => {
    daemon.failing.add("GET");
    renderPage();
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(`error: settings not loaded: ${HELD}`);
    expect(group("Permissions").getByRole("radio", { name: "accept edits" })).toBeChecked();
    daemon.failing.delete("GET");
    await userEvent.click(within(alert).getByRole("button", { name: "retry" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("moves settings saved in this browser to the daemon once", async () => {
    localStorage.setItem("th-settings", JSON.stringify({ ...DEFAULT_SETTINGS, agentsAtOnce: 11, theme: "x" }));
    await renderLoaded();
    await waitFor(() => expect(daemon.settings().agentsAtOnce).toBe(11));
    expect(screen.getByRole("group", { name: "Agents at once" })).toHaveTextContent("11");
    expect(daemon.puts()).toEqual([["/api/settings/agentsAtOnce", { value: 11 }]]);
    expect(localStorage.getItem("th-settings")).toBeNull();
  });
});
