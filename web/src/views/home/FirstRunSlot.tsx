import { NO_REPOS_TEXT } from "../../repos/copy";
import { useAddRepo, type AddRepoFlow } from "../../repos/useAddRepo";
import { SEARCH_HINT } from "../../shell/globalKeys";
import { useKeyHints, type KeyHint } from "../../shell/keyHints";
import { Button } from "../../ui/Button";
import { EmptyState } from "../../ui/EmptyState";
import { AddRepoModal } from "../repos/AddRepoModal";
import { FORM_HINTS } from "../repos/useRepoKeys";

const FIRST_RUN_KEYS: readonly KeyHint[] = [SEARCH_HINT];

function AddAction({ flow }: { readonly flow: AddRepoFlow }) {
  return (
    <>
      <Button variant="primary" aria-expanded={flow.formOpen} onClick={flow.begin}>
        add repository…
      </Button>
      {flow.formOpen ? (
        <AddRepoModal
          busy={flow.busy}
          error={flow.error}
          focusToken={flow.focusToken}
          onBrowse={flow.desktop ? flow.browse : undefined}
          onSubmit={(input) => void flow.submit(input)}
          onCancel={flow.cancel}
        />
      ) : null}
    </>
  );
}

export function FirstRunSlot() {
  const flow = useAddRepo();
  useKeyHints(flow.formOpen ? FORM_HINTS : FIRST_RUN_KEYS);
  return (
    <EmptyState logo title="add your first repository" action={<AddAction flow={flow} />}>
      {NO_REPOS_TEXT}
    </EmptyState>
  );
}
