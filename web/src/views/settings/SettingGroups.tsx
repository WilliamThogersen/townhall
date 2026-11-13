import { useState } from "react";
import { toMessage } from "../../api/errors";
import { useContributions } from "../../data";
import { AGENTS_AT_ONCE_MAX, AGENTS_AT_ONCE_MIN } from "../../settings/settings";
import { useSettings } from "../../settings/useSettings";
import { useTheme } from "../../theme/useTheme";
import { Segmented } from "../../ui/Segmented";
import { Stepper } from "../../ui/Stepper";
import { Switch } from "../../ui/Switch";
import { AccountsSection } from "./AccountsSection";
import { ClonesDirRow } from "./ClonesDirRow";
import { ProfileSection } from "./ProfileSection";
import { SettingGroup, SettingRow } from "./SettingRow";
import { PermissionSuggestions } from "./PermissionSuggestions";
import { HELP, NOTIFY_CHOICES, PERMISSION_TIMEOUT_CHOICES, REPO_UNCLEAR_CHOICES, THEME_CHOICES } from "./settingsCopy";

export function AgentsGroup() {
  const { settings, update } = useSettings();
  return (
    <SettingGroup title="agents">
      <SettingRow label="agents at once" help={HELP.agentsAtOnce}>
        <Stepper
          label="Agents at once"
          value={settings.agentsAtOnce}
          min={AGENTS_AT_ONCE_MIN}
          max={AGENTS_AT_ONCE_MAX}
          onChange={(agentsAtOnce) => update({ agentsAtOnce })}
        />
      </SettingRow>
    </SettingGroup>
  );
}

export function AdviserGroup() {
  const { settings, update } = useSettings();
  return (
    <SettingGroup title="adviser">
      <SettingRow label="adviser answers permissions" help={HELP.adviserPermissions}>
        <Switch
          label="Adviser answers permission prompts"
          checked={settings.adviserPermissions}
          onChange={(adviserPermissions) => update({ adviserPermissions })}
        />
      </SettingRow>
      {settings.adviserPermissions ? (
        <SettingRow label="adviser time limit" help={HELP.permissionTimeout}>
          <Segmented
            name="th-setting-permission-timeout"
            label="Adviser time limit"
            value={String(settings.permissionTimeoutSecs)}
            options={PERMISSION_TIMEOUT_CHOICES}
            onChange={(secs) => update({ permissionTimeoutSecs: Number(secs) })}
          />
        </SettingRow>
      ) : null}
      <PermissionSuggestions />
    </SettingGroup>
  );
}

export function ConversationsGroup() {
  const { settings, update } = useSettings();
  return (
    <SettingGroup title="conversations">
      <SettingRow label="when the repo is unclear" help={HELP.repoUnclear}>
        <Segmented
          name="th-setting-repo-unclear"
          label="When the repo is unclear"
          value={settings.repoUnclear}
          options={REPO_UNCLEAR_CHOICES}
          onChange={(repoUnclear) => update({ repoUnclear })}
        />
      </SettingRow>
      <SettingRow label="session panel" help={HELP.sessionPanel}>
        <Switch
          label="Session panel open by default"
          checked={settings.sessionPanelOpen}
          onChange={(sessionPanelOpen) => update({ sessionPanelOpen })}
        />
      </SettingRow>
    </SettingGroup>
  );
}

export function ReposGroup() {
  return (
    <SettingGroup title="repos">
      <ClonesDirRow />
    </SettingGroup>
  );
}

export function AccountsGroup() {
  return (
    <SettingGroup title="forge accounts">
      <AccountsSection />
    </SettingGroup>
  );
}

export function AboutYouGroup() {
  const { settings, update } = useSettings();
  return (
    <SettingGroup title="about you">
      <SettingRow label="learn about me" help={HELP.learnAboutMe}>
        <Switch
          label="Learn about me"
          checked={settings.learnAboutMe}
          onChange={(learnAboutMe) => update({ learnAboutMe })}
        />
      </SettingRow>
      <ProfileSection learnAboutMe={settings.learnAboutMe} />
    </SettingGroup>
  );
}

function PluginThemeRow() {
  const { themes, theme, chooseTheme } = useContributions();
  const [problem, setProblem] = useState<string | null>(null);
  if (themes.length === 0) return null;
  const options = [{ value: "", label: "none" }, ...themes.map((t) => ({ value: t.plugin, label: t.name }))];
  const choose = async (plugin: string) => {
    try {
      await chooseTheme(plugin === "" ? null : plugin);
      setProblem(null);
    } catch (e) {
      setProblem(`plugin theme not saved: ${toMessage(e)}`);
    }
  };
  return (
    <SettingRow label="plugin theme" help={problem ?? HELP.pluginTheme} helpTone={problem === null ? undefined : "danger"}>
      <Segmented
        name="th-setting-plugin-theme"
        label="Plugin theme"
        value={theme?.plugin ?? ""}
        options={options}
        onChange={(plugin) => void choose(plugin)}
      />
    </SettingRow>
  );
}

export function AppearanceGroup() {
  const { settings, update } = useSettings();
  const theme = useTheme();
  return (
    <SettingGroup title="appearance and notifications">
      <SettingRow label="theme" help={HELP.theme}>
        <Segmented
          name="th-setting-theme"
          label="Theme"
          value={theme.choice}
          options={THEME_CHOICES}
          onChange={theme.setChoice}
        />
      </SettingRow>
      <PluginThemeRow />
      <SettingRow label="notify me when" help={HELP.notifyWhen}>
        <Segmented
          name="th-setting-notify"
          label="Notify me when"
          value={settings.notifyWhen}
          options={NOTIFY_CHOICES}
          onChange={(notifyWhen) => update({ notifyWhen })}
        />
      </SettingRow>
    </SettingGroup>
  );
}
