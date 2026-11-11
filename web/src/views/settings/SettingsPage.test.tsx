import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { aSettingField, someContributions } from "../../api/fixtures";
import type { PluginContributions } from "../../api/types";
import { TestDataProvider } from "../../data/testing";
import { DEFAULT_SETTINGS, isSettingValue, SETTING_KEYS, type Settings } from "../../settings/settings";
import { createSettingsStore } from "../../settings/settingsStore";
import { SettingsStoreContext } from "../../settings/useSettings";
import { KeyHintsProvider, useCurrentKeyHints } from "../../shell/keyHints";
import { saveThemeChoice } from "../../theme/theme";
import { SettingsPage } from "./SettingsPage";

const HELD = "townhall.db is held by another townhall";
const VERSION = "0.2.0-alpha.1+dc699d6";

interface Call {
  readonly method: string;
  readonly url: string;
  readonly body: unknown;
}

function reply(status: number, data: unknown, error: string | null, warnings: readonly string[] = []): Response {
  return Response.json({ success: status < 400, data, error, ...(warnings.length === 0 ? {} : { warnings }) }, { status });
}

interface Found {
  readonly forge: "github" | "gitlab";
  readonly login: string;
  readonly host: string;
}

function fakeDaemon(
  initial: Partial<Settings> = {},
  found: readonly Found[] = [],
  start: PluginContributions = someContributions(),
) {
  let settings: Settings = { ...DEFAULT_SETTINGS, ...initial };
  let calls: readonly Call[] = [];
  let profile = "";
  let contributions = start;
  const failing = new Set<string>();
  const warnings: string[] = [];
  const answer = (
    method: string,
    url: string,
    body: { value?: unknown; text?: string; plugin?: string | null } | undefined,
  ): Response => {
    if (url === "/api/plugins/contributions") return reply(200, contributions, null);
    if (url === "/api/plugins/theme" && method === "PUT") {
      const chosen = contributions.themes.find((t) => t.plugin === body?.plugin);
      contributions = { ...contributions, theme: chosen ? { plugin: chosen.plugin, css: ":root {}\n" } : null };
      return reply(200, contributions, null);
    }
    if (url === "/api/forge-accounts/detected") {
      if (failing.has("detected")) return reply(400, null, "`gh` did not answer within 15 seconds");
      const registered = (f: Found) =>
        settings.forgeAccounts.some(
          (a) => a.forge === f.forge && a.host === f.host && a.login.toLowerCase() === f.login.toLowerCase(),
        );
      return reply(200, found.map((f) => ({ ...f, registered: registered(f) })), null, warnings);
    }
    if (url === "/api/info") {
      if (failing.has("info")) return reply(503, null, HELD);
      return reply(200, { home: "/h", repo: "/r", repoKey: "r-1", userName: null, version: VERSION }, null);
    }
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
    return answer(method, url, body as { value?: unknown; text?: string; plugin?: string | null } | undefined);
  });
  return {
    fetch,
    failing,
    warnings,
    settings: () => settings,
    profile: () => profile,
    puts: () => calls.filter((call) => call.method === "PUT").map((call) => [call.url, call.body]),
  };
}

let daemon = fakeDaemon();

function renderPage(children = <SettingsPage />) {
  const store = createSettingsStore();
  return render(
    <SettingsStoreContext value={store}>
      <TestDataProvider>{children}</TestDataProvider>
    </SettingsStoreContext>,
  );
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
  it("shows the six groups and how changes are saved", async () => {
    await renderLoaded();
    expect(screen.getByRole("heading", { level: 1, name: "settings" })).toBeInTheDocument();
    expect(screen.getByText("saved as you change them")).toBeInTheDocument();
    ["agents", "conversations", "repos", "forge accounts", "appearance and notifications", "about you"].forEach((name) => {
      expect(screen.getByRole("heading", { level: 2, name })).toBeInTheDocument();
    });
    const saves = screen.getAllByRole("button", { name: /^save/ });
    expect(saves).toHaveLength(1);
    expect(saves[0]?.closest(".th-setting-row")).toHaveTextContent("profile");
  });

  it("shows the full version below the last section, inside the settings column", async () => {
    await renderLoaded();
    const version = await screen.findByText(VERSION);
    const column = document.querySelector(".th-settings");
    expect(version.parentElement).toBe(column);
    expect(column?.lastElementChild).toBe(version);
    expect(version.previousElementSibling).toContainElement(screen.getByRole("heading", { level: 2, name: "about you" }));
  });

  it("shows no version while the daemon has not answered", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string, init?: RequestInit) =>
        url === "/api/info" ? new Promise<Response>(() => undefined) : daemon.fetch(url, init),
      ),
    );
    await renderLoaded();
    expect(screen.queryByText(/0\.2\.0/)).not.toBeInTheDocument();
  });

  it("shows no version when the daemon cannot answer", async () => {
    daemon.failing.add("info");
    await renderLoaded();
    await waitFor(() => expect(daemon.fetch).toHaveBeenCalledWith("/api/info", expect.anything()));
    expect(screen.queryByText(/0\.2\.0/)).not.toBeInTheDocument();
    expect(document.querySelector(".th-settings-version")).toBeNull();
  });

  it("has no permission rows: each agent type sets its own permissions", async () => {
    await renderLoaded();
    expect(screen.queryByRole("radiogroup", { name: "Permissions" })).not.toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: "Work without asking" })).not.toBeInTheDocument();
    expect(screen.queryByText("permissions", { selector: ".th-setting-label" })).not.toBeInTheDocument();
    const agents = screen.getByRole("heading", { level: 2, name: "agents" }).closest("section");
    const rows = [...(agents?.querySelectorAll(".th-setting-label") ?? [])].map((l) => l.textContent);
    expect(rows).toEqual(["agents at once"]);
  });

  it("renders every row", async () => {
    await renderLoaded();
    const labels = [...document.querySelectorAll(".th-setting-label")].map((label) => label.textContent);
    expect(labels).toEqual([
      "agents at once",
      "when the repo is unclear",
      "session panel",
      "clone folder",
      "accounts",
      "theme",
      "notify me when",
      "learn about me",
      "profile",
    ]);
  });

  it("saves the folder repos are cloned into, and refuses a relative one", async () => {
    await renderLoaded();
    const input = screen.getByRole("textbox", { name: "Clone folder" });
    expect(input).toHaveValue("");
    expect(input).toHaveAttribute("placeholder", "~/.townhall/repos");
    await userEvent.type(input, "projects");
    await userEvent.tab();
    expect(screen.getByRole("alert")).toHaveTextContent("use ~, ~/folder or an absolute path");
    expect(daemon.puts()).toEqual([]);
    await userEvent.clear(input);
    await userEvent.type(input, "~/Developer");
    await userEvent.tab();
    await waitFor(() => expect(lastPut()).toEqual(["/api/settings/clonesDir", { value: "~/Developer" }]));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await userEvent.clear(input);
    await userEvent.tab();
    await waitFor(() => expect(lastPut()).toEqual(["/api/settings/clonesDir", { value: "" }]));
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
    daemon = fakeDaemon({ agentsAtOnce: 9 });
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
    expect(screen.getByRole("group", { name: "Agents at once" })).toHaveTextContent("6");
    answer();
    await waitFor(() => expect(screen.getByRole("group", { name: "Agents at once" })).toHaveTextContent("9"));
    expect(document.querySelector(".th-settings")).toHaveAttribute("aria-busy", "false");
  });

  it("starts from the daemon's settings", async () => {
    daemon = fakeDaemon({ agentsAtOnce: 9, repoUnclear: "guess", notifyWhen: "failed" });
    vi.stubGlobal("fetch", daemon.fetch);
    await renderLoaded();
    expect(screen.getByRole("group", { name: "Agents at once" })).toHaveTextContent("9");
    expect(group("When the repo is unclear").getByRole("radio", { name: "guess" })).toBeChecked();
    expect(group("Notify me when").getByRole("radio", { name: "+ failed" })).toBeChecked();
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
    expect(screen.getByRole("group", { name: "Agents at once" })).toHaveTextContent("6");
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

describe("forge accounts", () => {
  const work = { forge: "github", login: "work-me", host: "github.com", label: "work" } as const;
  const glab = { forge: "gitlab", login: "me", host: "gitlab.corp.io", label: "corp gitlab" } as const;

  const fill = async (name: string, text: string) => {
    await userEvent.type(screen.getByRole("textbox", { name }), text);
  };
  const accounts = () => within(screen.getByRole("list", { name: "Registered accounts" }));

  it("says what happens without accounts", async () => {
    await renderLoaded();
    expect(screen.getByText(/no accounts yet/)).toHaveTextContent("gh and glab run as whoever is logged in");
    expect(screen.queryByRole("list", { name: "Registered accounts" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "add account" })).toBeDisabled();
  });

  it("lists the registered accounts with their forge, login and host", async () => {
    daemon = fakeDaemon({ forgeAccounts: [work, glab] });
    vi.stubGlobal("fetch", daemon.fetch);
    await renderLoaded();
    const rows = await accounts().findAllByRole("listitem");
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent("work");
    expect(rows[0]).toHaveTextContent("github · work-me@github.com");
    expect(rows[1]).toHaveTextContent("corp gitlab");
    expect(rows[1]).toHaveTextContent("gitlab · me@gitlab.corp.io");
  });

  it("adds a self-hosted gitlab account and saves the whole list", async () => {
    daemon = fakeDaemon({ forgeAccounts: [work] });
    vi.stubGlobal("fetch", daemon.fetch);
    await renderLoaded();
    await accounts().findAllByRole("listitem");
    await userEvent.click(within(screen.getByRole("radiogroup", { name: "Forge" })).getByRole("radio", { name: "gitlab" }));
    await fill("Login", "me");
    await fill("Host", "GitLab.Corp.io");
    await fill("Label", "corp gitlab");
    await userEvent.click(screen.getByRole("button", { name: "add account" }));
    await waitFor(() => expect(daemon.settings().forgeAccounts).toEqual([work, glab]));
    expect(lastPut()).toEqual(["/api/settings/forgeAccounts", { value: [work, glab] }]);
    expect(await accounts().findAllByRole("listitem")).toHaveLength(2);
    expect(screen.getByRole("textbox", { name: "Login" })).toHaveValue("");
  });

  it("fills in the default host and the login as the label", async () => {
    await renderLoaded();
    await fill("Login", "private-me");
    await userEvent.click(screen.getByRole("button", { name: "add account" }));
    const expected = [{ forge: "github", login: "private-me", host: "github.com", label: "private-me" }];
    await waitFor(() => expect(daemon.settings().forgeAccounts).toEqual(expected));
    expect(screen.getByRole("textbox", { name: "Host" })).toHaveAttribute("placeholder", "github.com");
  });

  it("removes an account and saves the rest", async () => {
    daemon = fakeDaemon({ forgeAccounts: [work, glab] });
    vi.stubGlobal("fetch", daemon.fetch);
    await renderLoaded();
    await userEvent.click(await screen.findByRole("button", { name: "remove work" }));
    await waitFor(() => expect(daemon.settings().forgeAccounts).toEqual([glab]));
    expect(await accounts().findAllByRole("listitem")).toHaveLength(1);
  });

  it("refuses the same account twice and a login that is not one, without asking the daemon", async () => {
    daemon = fakeDaemon({ forgeAccounts: [work] });
    vi.stubGlobal("fetch", daemon.fetch);
    await renderLoaded();
    await accounts().findAllByRole("listitem");
    await fill("Login", "Work-Me");
    await userEvent.click(screen.getByRole("button", { name: "add account" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("error: github account Work-Me on github.com is already registered");
    await userEvent.clear(screen.getByRole("textbox", { name: "Login" }));
    await fill("Login", "two words");
    await userEvent.click(screen.getByRole("button", { name: "add account" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("error: two words is not a login");
    expect(daemon.puts()).toEqual([]);
  });

  it("puts the account back when the daemon cannot save the list", async () => {
    await renderLoaded();
    daemon.failing.add("forgeAccounts");
    await fill("Login", "me");
    await userEvent.click(screen.getByRole("button", { name: "add account" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(`error: settings not saved: ${HELD}`);
    await waitFor(() => expect(screen.queryByRole("list", { name: "Registered accounts" })).not.toBeInTheDocument());
  });
});

describe("detected forge accounts", () => {
  const work = { forge: "github", login: "work-me", host: "github.com", label: "work" } as const;
  const found = [
    { forge: "github", login: "work-me", host: "github.com" },
    { forge: "github", login: "private-me", host: "github.com" },
    { forge: "github", login: "corp-me", host: "github.corp.io" },
    { forge: "gitlab", login: "gl-me", host: "gitlab.com" },
  ] as const;
  const entry = (f: (typeof found)[number]) => ({ ...f, label: f.login });

  const start = async (registered: Settings["forgeAccounts"], seen: readonly Found[] = found) => {
    daemon = fakeDaemon({ forgeAccounts: registered }, seen);
    vi.stubGlobal("fetch", daemon.fetch);
    await renderLoaded();
  };
  const detected = () => within(screen.getByRole("group", { name: "Detected accounts" }));

  it("offers the logged-in accounts that are not registered yet as chips, and none of the registered ones", async () => {
    await start([work]);
    await detected().findByRole("button", { name: "add private-me@github.com" });
    expect(detected().getByRole("button", { name: "add corp-me@github.corp.io" })).toBeInTheDocument();
    expect(detected().getByRole("button", { name: "add gl-me@gitlab.com" })).toBeInTheDocument();
    expect(detected().queryByRole("button", { name: "add work-me@github.com" })).not.toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Detected accounts" })).not.toHaveAttribute("data-prominent");
  });

  it("adds one account with one click, the login as its label, and the chip goes away", async () => {
    await start([work]);
    await userEvent.click(await detected().findByRole("button", { name: "add gl-me@gitlab.com" }));
    const gitlab = entry(found[3]);
    await waitFor(() => expect(daemon.settings().forgeAccounts).toEqual([work, gitlab]));
    expect(lastPut()).toEqual(["/api/settings/forgeAccounts", { value: [work, gitlab] }]);
    expect(detected().queryByRole("button", { name: "add gl-me@gitlab.com" })).not.toBeInTheDocument();
    expect(detected().getByRole("button", { name: "add private-me@github.com" })).toBeInTheDocument();
  });

  it("adds every detected account at once with one save", async () => {
    await start([work]);
    await userEvent.click(await detected().findByRole("button", { name: "add all" }));
    const rest = [entry(found[1]), entry(found[2]), entry(found[3])];
    await waitFor(() => expect(daemon.settings().forgeAccounts).toEqual([work, ...rest]));
    expect(daemon.puts()).toEqual([["/api/settings/forgeAccounts", { value: [work, ...rest] }]]);
    await waitFor(() => expect(screen.queryByRole("group", { name: "Detected accounts" })).not.toBeInTheDocument());
  });

  it("offers them prominently on the first visit, when no account is registered", async () => {
    await start([]);
    await detected().findByRole("button", { name: "add all" });
    const offer = screen.getByRole("group", { name: "Detected accounts" });
    expect(offer).toHaveAttribute("data-prominent", "true");
    expect(offer).toHaveTextContent("found 4 accounts logged in with gh and glab");
    expect(within(offer).getAllByRole("button", { name: /^add \w/ })).toHaveLength(5);
    await userEvent.click(within(offer).getByRole("button", { name: "add all" }));
    await waitFor(() => expect(daemon.settings().forgeAccounts).toEqual(found.map(entry)));
  });

  it("shows the two accounts of the real daemon answer as chips with add all, in the prominent box", async () => {
    daemon = fakeDaemon({}, []);
    daemon.fetch.mockImplementation(async (url: string) =>
      url === "/api/forge-accounts/detected"
        ? new Response(
            '{"success":true,"data":[{"forge":"github","login":"WilliamThogersen","host":"github.com","registered":false},{"forge":"github","login":"alice","host":"github.com","registered":false}],"error":null}',
          )
        : Response.json({ success: true, data: DEFAULT_SETTINGS, error: null }),
    );
    vi.stubGlobal("fetch", daemon.fetch);
    await renderLoaded();
    const offer = await screen.findByRole("group", { name: "Detected accounts" });
    expect(offer).toHaveAttribute("data-prominent", "true");
    expect(within(offer).getByRole("button", { name: "add WilliamThogersen@github.com" })).toBeInTheDocument();
    expect(within(offer).getByRole("button", { name: "add alice@github.com" })).toBeInTheDocument();
    expect(within(offer).getByRole("button", { name: "add all" })).toBeInTheDocument();
  });

  it("says it is looking while the daemon has not answered, instead of leaving a gap", async () => {
    daemon = fakeDaemon({}, found);
    daemon.fetch.mockImplementation(async (url: string) =>
      url === "/api/forge-accounts/detected"
        ? new Promise<Response>(() => undefined)
        : Response.json({ success: true, data: DEFAULT_SETTINGS, error: null }),
    );
    vi.stubGlobal("fetch", daemon.fetch);
    await renderLoaded();
    expect(screen.getByText(/looking for accounts logged in/)).toBeInTheDocument();
  });

  it("shows why a program found nothing next to the accounts that were found", async () => {
    daemon = fakeDaemon({}, [found[0]]);
    daemon.warnings.push("glab: no glab config at /home/me/.config/glab-cli/config.yml");
    vi.stubGlobal("fetch", daemon.fetch);
    await renderLoaded();
    const list = await screen.findByRole("list", { name: "Detection warnings" });
    expect(list).toHaveTextContent("glab: no glab config at /home/me/.config/glab-cli/config.yml");
    expect(screen.getByRole("group", { name: "Detected accounts" })).toBeInTheDocument();
  });

  it("says so when nobody is logged in", async () => {
    await start([], []);
    expect(await screen.findByText(/no logged-in accounts found/)).toHaveTextContent("gh auth login");
    expect(screen.getByText(/no accounts yet/)).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Detected accounts" })).not.toBeInTheDocument();
  });

  it("hides the offer once every detected account is registered", async () => {
    await start(found.map(entry));
    await waitFor(() => expect(daemon.fetch).toHaveBeenCalledWith("/api/forge-accounts/detected", expect.anything()));
    expect(screen.queryByText(/no logged-in accounts found/)).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Detected accounts" })).not.toBeInTheDocument();
  });

  it("says why detection failed and keeps the manual form working", async () => {
    daemon = fakeDaemon({}, found);
    daemon.failing.add("detected");
    vi.stubGlobal("fetch", daemon.fetch);
    await renderLoaded();
    expect(await screen.findByText(/could not look for logged-in accounts/)).toHaveTextContent(
      "`gh` did not answer within 15 seconds",
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await userEvent.type(screen.getByRole("textbox", { name: "Login" }), "me");
    expect(screen.getByRole("button", { name: "add account" })).toBeEnabled();
  });
});

describe("plugin settings and theme", () => {
  const slate = { plugin: "theme-slate@official", name: "theme-slate" };
  const dawn = { plugin: "dawn@acme", name: "dawn" };
  const withPlugins = (over: Partial<PluginContributions>) => {
    daemon = fakeDaemon({}, [], someContributions(over));
    vi.stubGlobal("fetch", daemon.fetch);
  };
  const headings = () => screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);

  it("adds no group and no row while no plugin contributes", async () => {
    await renderLoaded();
    expect(headings()).toEqual([
      "agents",
      "conversations",
      "repos",
      "forge accounts",
      "appearance and notifications",
      "about you",
    ]);
    expect(screen.queryByRole("radiogroup", { name: "Plugin theme" })).not.toBeInTheDocument();
  });

  it("adds a group for each plugin with settings, after appearance and before about you", async () => {
    withPlugins({
      settings: [{ plugin: "lint@acme-tools", name: "lint", fields: [aSettingField()] }],
    });
    await renderLoaded();
    expect(await screen.findByRole("heading", { level: 2, name: "lint (plugin)" })).toBeInTheDocument();
    expect(headings().slice(-3)).toEqual(["appearance and notifications", "lint (plugin)", "about you"]);
    const version = await screen.findByText(VERSION);
    expect(version.previousElementSibling).toContainElement(screen.getByRole("heading", { level: 2, name: "about you" }));
  });

  it("lets the user pick a plugin theme or none, saved on the daemon", async () => {
    withPlugins({ themes: [slate, dawn] });
    await renderLoaded();
    const themes = await screen.findByRole("radiogroup", { name: "Plugin theme" });
    expect(within(themes).getAllByRole("radio").map((r) => r.getAttribute("value"))).toEqual([
      "",
      slate.plugin,
      dawn.plugin,
    ]);
    expect(within(themes).getByRole("radio", { name: "none" })).toBeChecked();
    await userEvent.click(within(themes).getByRole("radio", { name: "theme-slate" }));
    await waitFor(() => expect(lastPut()).toEqual(["/api/plugins/theme", { plugin: slate.plugin }]));
    await waitFor(() => expect(within(themes).getByRole("radio", { name: "theme-slate" })).toBeChecked());
    await userEvent.click(within(themes).getByRole("radio", { name: "none" }));
    await waitFor(() => expect(lastPut()).toEqual(["/api/plugins/theme", { plugin: null }]));
    await waitFor(() => expect(within(themes).getByRole("radio", { name: "none" })).toBeChecked());
  });

  it("checks the theme that is active", async () => {
    withPlugins({ themes: [slate], theme: { plugin: slate.plugin, css: ":root {}\n" } });
    await renderLoaded();
    const themes = await screen.findByRole("radiogroup", { name: "Plugin theme" });
    expect(within(themes).getByRole("radio", { name: "theme-slate" })).toBeChecked();
  });
});
