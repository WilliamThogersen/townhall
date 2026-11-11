import type { ReactNode } from "react";
import type { PluginUiEntry } from "../../api/types";
import { PluginSlot } from "../../plugins/PluginSlot";
import { SettingGroup } from "./SettingRow";

const inGroup = (entry: PluginUiEntry, frame: ReactNode) => (
  <SettingGroup title={`${entry.title} (plugin)`}>{frame}</SettingGroup>
);

export function PluginSettingsFrames() {
  return <PluginSlot name="settings.group" wrap={inGroup} />;
}
