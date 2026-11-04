import { AgentsGroup, AppearanceGroup, ConversationsGroup } from "./SettingGroups";
import "./cap.css";

export function SettingsPage() {
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
