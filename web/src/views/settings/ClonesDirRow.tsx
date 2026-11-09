import { useState } from "react";
import { DEFAULT_CLONES_DIR } from "../../repos/addRepoDraft";
import { isClonesDirText } from "../../settings/settings";
import { useSettings } from "../../settings/useSettings";
import { SettingRow } from "./SettingRow";
import { HELP } from "./settingsCopy";

const BAD = "use ~, ~/folder or an absolute path";

/** The folder repos added by link are cloned into. Saved when the field loses focus or on enter. */
function ClonesDirInput({ saved, onSave }: { readonly saved: string; readonly onSave: (dir: string) => void }) {
  const [text, setText] = useState(saved);
  const [seen, setSeen] = useState(saved);
  const [bad, setBad] = useState(false);
  if (seen !== saved) {
    setSeen(saved);
    setText(saved);
  }
  const commit = () => {
    const dir = text.trim();
    if (!isClonesDirText(dir)) {
      setBad(true);
      return;
    }
    setBad(false);
    setText(dir);
    if (dir !== saved) onSave(dir);
  };
  return (
    <div className="th-setting-text">
      <input
        className="th-input"
        aria-label="Clone folder"
        aria-invalid={bad ? true : undefined}
        value={text}
        placeholder={DEFAULT_CLONES_DIR}
        autoComplete="off"
        autoCapitalize="off"
        spellCheck={false}
        onChange={(e) => {
          setText(e.target.value);
          setBad(false);
        }}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
        }}
      />
      {bad ? (
        <p className="th-field-error" role="alert">
          error: {BAD}
        </p>
      ) : null}
    </div>
  );
}

export function ClonesDirRow() {
  const { settings, update } = useSettings();
  return (
    <SettingRow label="clone folder" help={HELP.clonesDir}>
      <ClonesDirInput saved={settings.clonesDir} onSave={(clonesDir) => update({ clonesDir })} />
    </SettingRow>
  );
}
