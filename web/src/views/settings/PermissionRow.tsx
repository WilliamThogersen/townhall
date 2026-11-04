import { useId, useState } from "react";
import type { PermissionMode } from "../../settings/settings";
import { useSettings } from "../../settings/useSettings";
import { Segmented } from "../../ui/Segmented";
import { SettingRow } from "./SettingRow";
import { DANGEROUS_MODE, PERMISSION_CHOICES, PERMISSION_HELP } from "./settingsCopy";

interface DangerDetailsProps {
  readonly id: string;
  readonly open: boolean;
}

function DangerDetails({ id, open }: DangerDetailsProps) {
  return (
    <div className="th-setting-more" id={id} hidden={!open}>
      every edit and command runs straight away (<code>--dangerously-skip-permissions</code>). townhall
      and agents can never switch to this themselves.
    </div>
  );
}

export function PermissionRow() {
  const { settings, update } = useSettings();
  const [open, setOpen] = useState(false);
  const detailsId = useId();
  const mode = settings.permissionMode;
  const danger = mode === DANGEROUS_MODE;

  const choose = (next: PermissionMode) => {
    setOpen(false);
    update({ permissionMode: next });
  };

  const toggle = (
    <button
      type="button"
      className="th-setting-more-toggle"
      aria-controls={detailsId}
      aria-expanded={open}
      onClick={() => setOpen((was) => !was)}
    >
      {open ? "less" : "more"}
    </button>
  );

  return (
    <SettingRow
      label="permissions"
      help={
        <>
          {PERMISSION_HELP[mode]}
          {danger ? <> {toggle}</> : null}
        </>
      }
      helpTone={danger ? "danger" : undefined}
      details={danger ? <DangerDetails id={detailsId} open={open} /> : null}
    >
      <Segmented
        name="th-setting-permissions"
        label="Permissions"
        value={mode}
        options={PERMISSION_CHOICES}
        onChange={choose}
      />
    </SettingRow>
  );
}
