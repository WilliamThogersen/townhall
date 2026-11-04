import { NO_REPOS_TEXT } from "../../repos/copy";
import { useAddRepo, type AddRepoFlow } from "../../repos/useAddRepo";
import { Button } from "../../ui/Button";
import { EmptyState } from "../../ui/EmptyState";
import { AddRepoForm } from "../repos/AddRepoForm";

function AddAction({ flow }: { readonly flow: AddRepoFlow }) {
  if (flow.formOpen) {
    return (
      <AddRepoForm
        layout="stack"
        busy={flow.busy}
        error={flow.error}
        focusToken={flow.focusToken}
        onSubmit={(input) => void flow.submit(input)}
        onCancel={flow.cancel}
      />
    );
  }
  return (
    <>
      <Button variant="primary" busy={flow.busy} busyLabel="adding…" onClick={flow.begin}>
        add repository…
      </Button>
      {flow.error ? (
        <p className="th-field-error" role="alert">
          error: {flow.error}
        </p>
      ) : null}
    </>
  );
}

export function FirstRunSlot() {
  const flow = useAddRepo();
  return (
    <EmptyState logo title="add your first repository" action={<AddAction flow={flow} />}>
      {NO_REPOS_TEXT}
    </EmptyState>
  );
}
