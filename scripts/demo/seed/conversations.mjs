import { waitFor } from "./api.mjs";
import { sqlText } from "./db.mjs";
import { demoFile } from "./files.mjs";

const TURN_MS = 30000;
const NEW_TITLE = "new conversation";

async function settle(api, id, until) {
  const ready = async () => {
    const { conversation } = await api.get(`/conversations/${id}`);
    if (until === "thinking") return conversation.status === "thinking" && conversation.title !== NEW_TITLE ? conversation : null;
    return conversation.status === "yourTurn" ? conversation : null;
  };
  return waitFor(`conversation ${id} to settle`, ready, TURN_MS);
}

async function uploadAll(api, id, names) {
  const uploaded = [];
  for (const name of names ?? []) {
    uploaded.push(await api.upload(id, demoFile(name)));
  }
  return uploaded.map((attachment) => attachment.id);
}

function attachToFirstMessage(db, id, attachmentIds) {
  const firstYou = `(select min(id) from messages where conversation_id = ${sqlText(id)} and author = 'you')`;
  db.run([`update messages set attachments = ${sqlText(JSON.stringify(attachmentIds))} where conversation_id = ${sqlText(id)} and id = ${firstYou};`]);
}

async function sendRest(api, id, messages) {
  for (const message of messages) {
    const attachments = await uploadAll(api, id, message.files);
    await api.post(`/conversations/${id}/messages`, { text: message.text, attachments });
    await settle(api, id, message.until);
  }
}

export async function seedConversation(api, db, plan) {
  const [first, ...rest] = plan.messages;
  const created = await api.post("/conversations", { text: first.text, attachments: [] });
  const id = created.conversation.id;
  await settle(api, id, first.until);
  const firstFiles = await uploadAll(api, id, first.files);
  if (firstFiles.length > 0) attachToFirstMessage(db, id, firstFiles);
  await sendRest(api, id, rest);
  await uploadAll(api, id, plan.looseFiles);
  if (plan.title) await api.patch(`/conversations/${id}`, { title: plan.title });
  const { conversation } = await api.get(`/conversations/${id}`);
  return { key: plan.key, id, title: conversation.title };
}
