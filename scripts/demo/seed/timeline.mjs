import { sqlText } from "./db.mjs";

const MINUTE_MS = 60000;
const MESSAGE_KINDS = new Set(["you", "townhall"]);

const iso = (ms) => new Date(ms).toISOString();

function spread(count, start, end) {
  if (count <= 1) return [end];
  return Array.from({ length: count }, (_, i) => Math.round(start + ((end - start) * i) / (count - 1)));
}

function entryUpdates(id, entries, times) {
  return entries.map((entry, i) => {
    const table = MESSAGE_KINDS.has(entry.kind) ? "messages" : "events";
    return `update ${table} set at = ${sqlText(iso(times[i]))} where conversation_id = ${sqlText(id)} and id = ${entry.id};`;
  });
}

function taskUpdates(entries, times) {
  return entries.flatMap((entry, i) => (entry.kind === "tasksCreated"
    ? entry.tasks.map((name) => `update tasks set created_at = ${sqlText(iso(times[i]))} where name = ${sqlText(name)};`)
    : []));
}

function attachmentUpdates(id, entries, times, attachments) {
  const sentAt = new Map(entries.flatMap((entry, i) => (entry.kind === "you" ? entry.attachments.map((a) => [a, times[i]]) : [])));
  const fallback = times[times.length - 1];
  return attachments.map((attachment) => {
    const at = iso(sentAt.get(attachment.id) ?? fallback);
    return `update attachments set created_at = ${sqlText(at)} where conversation_id = ${sqlText(id)} and id = ${sqlText(attachment.id)};`;
  });
}

export function backdate(detail, plan, now) {
  const { conversation, entries, attachments } = detail;
  const end = now - plan.minutesAgo * MINUTE_MS;
  const times = spread(entries.length, end - plan.spanMinutes * MINUTE_MS, end);
  const first = iso(times[0] ?? end);
  const last = iso(times[times.length - 1] ?? end);
  return [
    ...entryUpdates(conversation.id, entries, times),
    ...taskUpdates(entries, times),
    ...attachmentUpdates(conversation.id, entries, times, attachments),
    `update conversations set created_at = ${sqlText(first)}, updated_at = ${sqlText(last)} where id = ${sqlText(conversation.id)};`,
  ];
}
