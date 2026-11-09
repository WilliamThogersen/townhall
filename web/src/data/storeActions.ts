import {
  endConversation,
  patchConversation,
  sendMessage,
  startConversation,
  stopTurn,
} from "../api/conversations";
import { addRepo, patchRepo, putRepoAccounts, removeRepo } from "../api/repos";
import type { AddRepoInput, Conversation, PatchRepoInput, PutRepoAccountsInput, Repo, ServerEvent } from "../api/types";
import { applyRepoEdited, applyStarted, upsertRepo, withoutRepo } from "./reducers";
import { mapResource, type DataState } from "./state";
import type { ConversationActions } from "./types";

export interface StoreAccess {
  readonly update: (change: (state: DataState) => DataState) => void;
  readonly apply: (event: ServerEvent) => void;
}

export interface RepoActions {
  readonly addRepo: (input: AddRepoInput) => Promise<Repo>;
  readonly removeRepo: (name: string, deleteClone?: boolean) => Promise<void>;
  readonly editRepo: (name: string, input: PatchRepoInput) => Promise<Repo>;
  readonly setRepoAccounts: (name: string, input: PutRepoAccountsInput) => Promise<Repo>;
}

export function createConversationActions({ update, apply }: StoreAccess): ConversationActions {
  const settle = async (answer: Promise<Conversation>): Promise<Conversation> => {
    const conversation = await answer;
    apply({ type: "conversationUpdated", conversation });
    return conversation;
  };
  return {
    start: async (input, files = []) => {
      const detail = await startConversation(input, files);
      update((state) => applyStarted(state, detail));
      return detail;
    },
    send: async (id, input) => {
      const entry = await sendMessage(id, input);
      apply({ type: "entryAppended", conversationId: id, entry });
      return entry;
    },
    stop: (id) => settle(stopTurn(id)),
    end: (id) => settle(endConversation(id)),
    patch: (id, patch) => settle(patchConversation(id, patch)),
  };
}

export function createRepoActions({ update }: StoreAccess): RepoActions {
  return {
    addRepo: async (input) => {
      const repo = await addRepo(input);
      update((state) => ({ ...state, repos: mapResource(state.repos, (list) => upsertRepo(list, repo)) }));
      return repo;
    },
    removeRepo: async (name, deleteClone) => {
      await (deleteClone ? removeRepo(name, true) : removeRepo(name));
      update((state) => ({ ...state, repos: mapResource(state.repos, (list) => withoutRepo(list, name)) }));
    },
    editRepo: async (name, input) => {
      const repo = await patchRepo(name, input);
      update((state) => applyRepoEdited(state, name, repo));
      return repo;
    },
    setRepoAccounts: async (name, input) => {
      const repo = await putRepoAccounts(name, input);
      update((state) => ({ ...state, repos: mapResource(state.repos, (list) => upsertRepo(list, repo)) }));
      return repo;
    },
  };
}
