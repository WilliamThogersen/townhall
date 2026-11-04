import { useKeyHints, type KeyHint } from "../../shell/keyHints";
import { AgentsGroup, AppearanceGroup, ConversationsGroup } from "./SettingGroups";
import "./cap.css";

export const SETTINGS_HINTS: readonly KeyHint[] = [
  { keys: "tab", label: "next setting" },
  { keys: "← →", label: "change choice" },
];

export function SettingsPage() {
  useKeyHints(SETTINGS_HINTS);
  return (
    <div className="th-page">
      <header className="th-page-head">
        <h1>settings</h1>
        <span className="th-cap">saved as you change them</span>
      </header>
      <div className="th-page-body">
        <div className="th-settings">
          <AgentsGroup />
          <ConversationsGroup />
          <AppearanceGroup />
        </div>
      </div>
    </div>
  );
}
