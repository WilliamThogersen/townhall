import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { toMessage } from "../../api/errors";
import { deleteProfile, getProfile, MAX_PROFILE_CHARS, putProfile } from "../../api/profile";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { SettingRow } from "./SettingRow";
import { HELP } from "./settingsCopy";

type Phase = "loading" | "ready" | "failed";
type Busy = "save" | "reset" | null;

interface ResetConfirmProps {
  readonly busy: boolean;
  readonly onCancel: () => void;
  readonly onConfirm: () => void;
}

function ResetConfirm({ busy, onCancel, onConfirm }: ResetConfirmProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  useEffect(() => cancelRef.current?.focus(), []);
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "Escape" || busy) return;
    e.preventDefault();
    onCancel();
  };
  return (
    <div className="th-profile-actions" role="group" aria-label="reset profile" onKeyDown={onKeyDown}>
      <span className="th-confirm">
        <span>reset your profile? the saved text is deleted for good</span>
      </span>
      <Button ref={cancelRef} variant="ghost" size="sm" disabled={busy} onClick={onCancel}>
        cancel
      </Button>
      <Button variant="danger-solid" size="sm" busy={busy} busyLabel="resetting…" onClick={onConfirm}>
        reset
      </Button>
    </div>
  );
}

export function ProfileSection({ learnAboutMe }: { readonly learnAboutMe: boolean }) {
  const [phase, setPhase] = useState<Phase>("loading");
  const [saved, setSaved] = useState("");
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState<Busy>(null);
  const [confirming, setConfirming] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    getProfile(controller.signal).then(
      (profile) => {
        setSaved(profile.text);
        setDraft(profile.text);
        setProblem(null);
        setPhase("ready");
      },
      (e: unknown) => {
        if (controller.signal.aborted) return;
        setPhase("failed");
        setProblem(`profile not loaded: ${toMessage(e)}`);
      },
    );
    return () => controller.abort();
  }, [attempt]);

  const retry = useCallback(() => {
    setPhase("loading");
    setAttempt((n) => n + 1);
  }, []);

  const save = async () => {
    setBusy("save");
    try {
      const profile = await putProfile(draft);
      setSaved(profile.text);
      setDraft(profile.text);
      setProblem(null);
    } catch (e) {
      setProblem(`profile not saved: ${toMessage(e)}`);
    } finally {
      setBusy(null);
    }
  };

  const reset = async () => {
    setBusy("reset");
    try {
      await deleteProfile();
      setSaved("");
      setDraft("");
      setProblem(null);
    } catch (e) {
      setProblem(`profile not reset: ${toMessage(e)}`);
    } finally {
      setBusy(null);
      setConfirming(false);
    }
  };

  const ready = phase === "ready";
  const dirty = draft !== saved;
  const action =
    phase === "failed" ? (
      <Button size="sm" onClick={retry}>
        retry
      </Button>
    ) : (
      <Button size="sm" onClick={() => setProblem(null)}>
        dismiss
      </Button>
    );

  return (
    <SettingRow wide label="profile" help={learnAboutMe ? HELP.profileOn : HELP.profileOff}>
      {problem === null ? null : (
        <Banner level="error" action={action}>
          {problem}
        </Banner>
      )}
      {ready && saved === "" ? (
        <p className="th-cap">no profile yet: write how you work, your tone of voice and your typical flow</p>
      ) : null}
      <textarea
        className="th-input th-profile-input"
        aria-label="Profile"
        maxLength={MAX_PROFILE_CHARS}
        value={draft}
        disabled={!ready || busy !== null}
        placeholder="how you work, your tone of voice, your typical flow"
        onChange={(e) => setDraft(e.target.value)}
      />
      <div className="th-profile-actions">
        <Button
          size="sm"
          busy={busy === "save"}
          busyLabel="saving…"
          disabled={!dirty || !ready || busy !== null}
          onClick={() => void save()}
        >
          save
        </Button>
        {dirty ? <span className="th-cap">unsaved changes</span> : null}
        {saved !== "" && !confirming ? (
          <Button variant="danger" size="sm" disabled={busy !== null} onClick={() => setConfirming(true)}>
            reset
          </Button>
        ) : null}
      </div>
      {confirming ? (
        <ResetConfirm busy={busy === "reset"} onCancel={() => setConfirming(false)} onConfirm={() => void reset()} />
      ) : null}
    </SettingRow>
  );
}
