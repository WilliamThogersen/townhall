import { beforeEach, describe, expect, it } from "vitest";
import { clearAllQueues, enqueue, readQueue, removeQueued, type QueuedMessage } from "./queue";

const draft = (text: string): Omit<QueuedMessage, "id"> => ({ text, attachmentIds: [], reply: null });

describe("message queue", () => {
  beforeEach(clearAllQueues);

  it("is empty until something is queued", () => {
    expect(readQueue("a")).toEqual([]);
    expect(readQueue(null)).toEqual([]);
  });

  it("keeps the order in which messages were queued", () => {
    enqueue("a", draft("one"));
    enqueue("a", draft("two"));
    expect(readQueue("a").map((item) => item.text)).toEqual(["one", "two"]);
  });

  it("keeps a queue per conversation", () => {
    enqueue("a", draft("for a"));
    enqueue("b", draft("for b"));
    enqueue(null, draft("for home"));
    expect(readQueue("a").map((item) => item.text)).toEqual(["for a"]);
    expect(readQueue("b").map((item) => item.text)).toEqual(["for b"]);
    expect(readQueue(null).map((item) => item.text)).toEqual(["for home"]);
  });

  it("gives every message its own id", () => {
    const first = enqueue("a", draft("one"));
    const second = enqueue("a", draft("two"));
    expect(first.id).not.toBe(second.id);
  });

  it("removes one message by id and hands it back", () => {
    const first = enqueue("a", draft("one"));
    enqueue("a", draft("two"));
    expect(removeQueued("a", first.id)).toEqual(first);
    expect(readQueue("a").map((item) => item.text)).toEqual(["two"]);
  });

  it("answers null when the message is already gone", () => {
    expect(removeQueued("a", 99)).toBeNull();
  });

  it("puts a message back at the end with the id it had", () => {
    const first = enqueue("a", draft("one"));
    enqueue("a", draft("two"));
    removeQueued("a", first.id);
    enqueue("a", first);
    expect(readQueue("a").map((item) => item.id)).toEqual([expect.any(Number), first.id]);
  });

  it("returns the same array until it changes", () => {
    enqueue("a", draft("one"));
    expect(readQueue("a")).toBe(readQueue("a"));
    expect(readQueue("none")).toBe(readQueue("other"));
  });
});
