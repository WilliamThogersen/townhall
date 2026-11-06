import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { clearAllReplies, readReply, useReply, writeReply, type ReplyTarget } from "./replies";

const asked: ReplyTarget = { id: 4, author: "townhall", excerpt: "which repo?" };

describe("replies", () => {
  beforeEach(clearAllReplies);

  it("keeps the entry being answered per conversation", () => {
    writeReply("a", asked);
    expect(readReply("a")).toEqual(asked);
    expect(readReply("b")).toBeNull();
    expect(readReply(null)).toBeNull();
  });

  it("clears a reply by writing null", () => {
    writeReply("a", asked);
    writeReply("a", null);
    expect(readReply("a")).toBeNull();
  });

  it("tells a mounted hook when its conversation's reply changes, and only then", () => {
    const { result } = renderHook(() => useReply("a"));
    expect(result.current).toBeNull();
    act(() => writeReply("b", asked));
    expect(result.current).toBeNull();
    act(() => writeReply("a", asked));
    expect(result.current).toEqual(asked);
    act(() => writeReply("a", null));
    expect(result.current).toBeNull();
  });
});
