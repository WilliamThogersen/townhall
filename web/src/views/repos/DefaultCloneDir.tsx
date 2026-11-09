import { useState } from "react";
import { DEFAULT_CLONES_DIR } from "../../repos/addRepoDraft";
import { isClonesDirText } from "../../settings/settings";
import { useSettings } from "../../settings/useSettings";

const BAD = "use ~, ~/folder or an absolute path";
const HELP = "where a repo added by link is cloned, as <folder>/<repo>. you can pick another folder each time you add one.";

export function DefaultCloneDir() {
  const { settings, update } = useSettings();
  const saved = settings.clonesDir;
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
    if (dir !== saved) update({ clonesDir: dir });
  };
  return (
    <div className="th-repo-default-dir">
      <label className="th-repo-field">
        <span>default clone folder</span>
        <input
          className="th-input"
          aria-label="default clone folder"
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
      </label>
      {bad ? (
        <p className="th-field-error" role="alert">
          error: {BAD}
        </p>
      ) : (
        <p className="th-cap">{HELP}</p>
      )}
    </div>
  );
}
