import { AGENTS_AT_ONCE_MAX, AGENTS_AT_ONCE_MIN } from "../../settings/settings";
import { useSettings } from "../../settings/useSettings";
import { useTheme } from "../../theme/useTheme";
import { Segmented } from "../../ui/Segmented";
import { Stepper } from "../../ui/Stepper";
import { Switch } from "../../ui/Switch";
import { PermissionRow } from "./PermissionRow";
import { SettingGroup, SettingRow } from "./SettingRow";
import { HELP, NOTIFY_CHOICES, REPO_UNCLEAR_CHOICES, THEME_CHOICES } from "./settingsCopy";

export function AgentsGroup() {
  const { settings, update } = useSettings();
  return (
    <SettingGroup title="agents">
      <PermissionRow />
      <SettingRow label="work without asking" help={HELP.workWithoutAsking}>
        <Switch
          label="Work without asking"
          checked={settings.workWithoutAsking}
          onChange={(workWithoutAsking) => update({ workWithoutAsking })}
        />
      </SettingRow>
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
