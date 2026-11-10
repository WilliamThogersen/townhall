import { useState } from "react";
import { toMessage } from "../../api/errors";
import type { PluginSettingField, PluginSettingsGroup, SettingFieldValue } from "../../api/types";
import { useContributions } from "../../data";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { Segmented } from "../../ui/Segmented";
import { Stepper } from "../../ui/Stepper";
import { Switch } from "../../ui/Switch";
import { SettingGroup, SettingRow } from "./SettingRow";

const SEGMENTED_MAX = 4;

interface Controls {
  readonly save: (value: SettingFieldValue) => void;
}

interface FieldProps extends Controls {
  readonly field: PluginSettingField;
}

function TextControl({ field, save }: FieldProps) {
  const shown = field.secret ? "" : String(field.value ?? "");
  const [draft, setDraft] = useState(shown);
  const [source, setSource] = useState(shown);
  if (source !== shown) {
    setSource(shown);
    setDraft(shown);
  }
  const dirty = field.secret ? draft !== "" : draft !== shown;
  const placeholder = field.secret ? (field.set ? "saved: type to replace it" : "not set") : undefined;
  return (
    <div className="th-plugin-setting-text">
      <input
        className="th-input"
        type={field.secret ? "password" : "text"}
        autoComplete="off"
        aria-label={field.label}
        placeholder={placeholder}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
      />
      <Button
        size="sm"
        aria-label={`save ${field.label}`}
        disabled={!dirty}
        onClick={() => {
          save(draft);
          if (field.secret) setDraft("");
        }}
      >
        save
      </Button>
    </div>
  );
}

function Control({ field, save }: FieldProps) {
  const choices = field.choices ?? [];
  switch (field.type) {
    case "boolean":
      return <Switch label={field.label} checked={field.value === true} onChange={save} />;
    case "integer":
      return (
        <Stepper
          label={field.label}
          value={typeof field.value === "number" ? field.value : Number(field.default)}
          min={field.min ?? Number.MIN_SAFE_INTEGER}
          max={field.max ?? Number.MAX_SAFE_INTEGER}
          onChange={save}
        />
      );
    case "enum":
      return choices.length <= SEGMENTED_MAX ? (
        <Segmented
          name={`th-plugin-setting-${field.key}`}
          label={field.label}
          value={String(field.value)}
          options={choices.map((choice) => ({ value: choice, label: choice }))}
          onChange={save}
        />
      ) : (
        <select
          className="th-input"
          aria-label={field.label}
          value={String(field.value)}
          onChange={(e) => save(e.target.value)}
        >
          {choices.map((choice) => (
            <option key={choice} value={choice}>
              {choice}
            </option>
          ))}
        </select>
      );
    case "string":
      return <TextControl field={field} save={save} />;
  }
}

interface GroupProps {
  readonly group: PluginSettingsGroup;
}

function Group({ group }: GroupProps) {
  const { setSetting, resetSetting } = useContributions();
  const [problem, setProblem] = useState<string | null>(null);
  const run = async (what: () => Promise<unknown>) => {
    try {
      await what();
      setProblem(null);
    } catch (e) {
      setProblem(`${group.name} settings not saved: ${toMessage(e)}`);
    }
  };
  return (
    <SettingGroup title={`${group.name} (plugin)`}>
      {problem === null ? null : (
        <Banner
          level="error"
          action={
            <Button size="sm" onClick={() => setProblem(null)}>
              dismiss
            </Button>
          }
        >
          {problem}
        </Banner>
      )}
      {group.fields.map((field) => (
        <SettingRow key={field.key} label={field.label} help={field.description ?? ""}>
          <div className="th-plugin-setting-control">
            <Control field={field} save={(value) => void run(() => setSetting(group.plugin, field.key, value))} />
            {field.set ? (
              <Button
                variant="ghost"
                size="sm"
                aria-label={`reset ${field.label}`}
                onClick={() => void run(() => resetSetting(group.plugin, field.key))}
              >
                reset
              </Button>
            ) : null}
          </div>
        </SettingRow>
      ))}
    </SettingGroup>
  );
}

export function PluginSettingsGroups() {
  const { settings } = useContributions();
  return (
    <>
      {settings.map((group) => (
        <Group key={group.plugin} group={group} />
      ))}
    </>
  );
}
