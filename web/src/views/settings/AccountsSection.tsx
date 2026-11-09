import { useState, type ChangeEvent, type FormEvent } from "react";
import type { ForgeAccount, ForgeKind } from "../../settings/settings";
import { useSettings } from "../../settings/useSettings";
import { Button } from "../../ui/Button";
import { Segmented } from "../../ui/Segmented";
import { DetectedAccounts } from "./DetectedAccounts";
import { SettingRow } from "./SettingRow";
import { FORGE_CHOICES, HELP } from "./settingsCopy";
import "./accounts.css";

const DEFAULT_HOSTS: Readonly<Record<ForgeKind, string>> = { github: "github.com", gitlab: "gitlab.com" };
const LOGIN = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;
const HOST = /^[a-z0-9][a-z0-9.-]*(:\d{1,5})?$/;

interface Draft {
  readonly forge: ForgeKind;
  readonly login: string;
  readonly host: string;
  readonly label: string;
}

const EMPTY_DRAFT: Draft = { forge: "github", login: "", host: "", label: "" };

export function accountFrom(draft: Draft, known: readonly ForgeAccount[]): ForgeAccount | string {
  const login = draft.login.trim();
  const host = (draft.host.trim() || DEFAULT_HOSTS[draft.forge]).toLowerCase();
  if (!LOGIN.test(login)) return `${login} is not a login`;
  if (!HOST.test(host)) return `${host} is not a host name, like gitlab.example.com`;
  const twice = known.some(
    (a) => a.forge === draft.forge && a.host === host && a.login.toLowerCase() === login.toLowerCase(),
  );
  if (twice) return `${draft.forge} account ${login} on ${host} is already registered`;
  return { forge: draft.forge, login, host, label: draft.label.trim() || login };
}

interface FieldProps {
  readonly label: string;
  readonly value: string;
  readonly placeholder: string;
  readonly onChange: (value: string) => void;
}

function Field({ label, value, placeholder, onChange }: FieldProps) {
  return (
    <label className="th-account-field">
      <span>{label.toLowerCase()}</span>
      <input
        className="th-input"
        aria-label={label}
        value={value}
        placeholder={placeholder}
        autoComplete="off"
        autoCapitalize="off"
        spellCheck={false}
        onChange={(e: ChangeEvent<HTMLInputElement>) => onChange(e.target.value)}
      />
    </label>
  );
}

function AccountList({ accounts, onRemove }: { readonly accounts: readonly ForgeAccount[]; readonly onRemove: (a: ForgeAccount) => void }) {
  if (accounts.length === 0) {
    return <p className="th-cap">no accounts yet: gh and glab run as whoever is logged in</p>;
  }
  return (
    <ul className="th-accounts" aria-label="Registered accounts">
      {accounts.map((account) => (
        <li key={`${account.forge}/${account.host}/${account.login}`} className="th-account">
          <b>{account.label}</b>
          <span className="th-cap">{`${account.forge} · ${account.login}@${account.host}`}</span>
          <Button variant="ghost" size="sm" aria-label={`remove ${account.label}`} onClick={() => onRemove(account)}>
            remove
          </Button>
        </li>
      ))}
    </ul>
  );
}

export function AccountsSection() {
  const { settings, update } = useSettings();
  const [draft, setDraft] = useState<Draft>(EMPTY_DRAFT);
  const [error, setError] = useState<string | null>(null);
  const accounts = settings.forgeAccounts;
  const set = (key: keyof Draft) => (value: string) => setDraft((current) => ({ ...current, [key]: value }));
  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const account = accountFrom(draft, accounts);
    if (typeof account === "string") {
      setError(account);
      return;
    }
    setError(null);
    update({ forgeAccounts: [...accounts, account] });
    setDraft({ ...EMPTY_DRAFT, forge: draft.forge });
  };
  const remove = (gone: ForgeAccount) => update({ forgeAccounts: accounts.filter((a) => a !== gone) });
  return (
    <SettingRow wide label="accounts" help={HELP.forgeAccounts}>
      <DetectedAccounts accounts={accounts} onAdd={(added) => update({ forgeAccounts: [...accounts, ...added] })} />
      <p className="th-cap th-account-login">
        log in first: <code>gh auth login</code> or <code>glab auth login</code>
      </p>
      <AccountList accounts={accounts} onRemove={remove} />
      <form className="th-account-add" aria-label="add account" onSubmit={submit}>
        <div className="th-account-field">
          <span>forge</span>
          <Segmented
            name="th-setting-account-forge"
            label="Forge"
            value={draft.forge}
            options={FORGE_CHOICES}
            onChange={(forge) => setDraft((current) => ({ ...current, forge }))}
          />
        </div>
        <Field label="Login" value={draft.login} placeholder="your login" onChange={set("login")} />
        <Field label="Host" value={draft.host} placeholder={DEFAULT_HOSTS[draft.forge]} onChange={set("host")} />
        <Field label="Label" value={draft.label} placeholder="the login" onChange={set("label")} />
        <Button type="submit" aria-label="add account" disabled={draft.login.trim() === ""}>
          add
        </Button>
        {error === null ? null : (
          <p className="th-field-error th-account-error" role="alert">
            error: {error}
          </p>
        )}
      </form>
    </SettingRow>
  );
}
