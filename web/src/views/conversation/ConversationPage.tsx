import { useConversation } from "../../data";
import { Link, PATHS } from "../../router";
import { NEW_CONVERSATION_HINT, SEARCH_HINT } from "../../shell/globalKeys";
import { useKeyHints } from "../../shell/keyHints";
import { Button } from "../../ui/Button";
import { EmptyState } from "../../ui/EmptyState";
import { Loading } from "../../ui/Loading";
import { ConversationView } from "./ConversationView";
import { useAttachmentRefresh } from "./useAttachmentRefresh";

const WAITING_KEYS = [NEW_CONVERSATION_HINT, SEARCH_HINT];
const DAEMON_DOWN = "cannot reach the daemon";

export function isNotFound(error: string): boolean {
  return !error.startsWith(DAEMON_DOWN) && /not found$/.test(error);
}

interface ProblemProps {
  readonly id: string;
  readonly error: string;
  readonly onRetry: () => void;
}

function ConversationProblem({ id, error, onRetry }: ProblemProps) {
  useKeyHints(WAITING_KEYS);
  if (isNotFound(error)) {
    const home = (
      <Link className="th-btn" to={PATHS.home}>
        new conversation
      </Link>
    );
    return (
      <EmptyState title="conversation not found" action={home}>
        error: conversation {id} not found
      </EmptyState>
    );
  }
  const retry = (
    <Button size="sm" onClick={onRetry}>
      retry
    </Button>
  );
  if (error.startsWith(DAEMON_DOWN)) {
    return (
      <EmptyState title="cannot reach the daemon" action={retry}>
        info: the conversation opens once the daemon answers
      </EmptyState>
    );
  }
  return (
    <EmptyState title="cannot open this conversation" action={retry}>
      error: {error}
    </EmptyState>
  );
}

function ConversationLoading() {
  useKeyHints(WAITING_KEYS);
  return (
    <div className="th-empty">
      <Loading verb="opening conversation" />
    </div>
  );
}

export function ConversationPage({ id }: { readonly id: string }) {
  const { detail, loading, error, reload } = useConversation(id);
  useAttachmentRefresh(detail, reload);
  if (detail !== null) return <ConversationView key={id} id={id} detail={detail} />;
  if (loading || error === null) return <ConversationLoading />;
  return <ConversationProblem id={id} error={error} onRetry={reload} />;
}
