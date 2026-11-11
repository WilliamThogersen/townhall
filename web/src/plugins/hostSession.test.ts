import { beforeEach, describe, expect, it, vi } from "vitest";
import { forgetHostSession, hostSession } from "./hostSession";

const minted = (plugin: string) => ({ plugin, version: "1.0.0", token: `tok-${plugin}`, scopes: ["storage"] });

beforeEach(() => {
  forgetHostSession("a@m");
  forgetHostSession("b@m");
});

describe("hostSession", () => {
  it("mints once per plugin and keeps the answer in memory", async () => {
    const mint = vi.fn(async (id: string) => minted(id));
    const [one, two] = await Promise.all([hostSession("a@m", mint), hostSession("a@m", mint)]);
    expect(one).toEqual(minted("a@m"));
    expect(two).toBe(one);
    await hostSession("a@m", mint);
    expect(mint).toHaveBeenCalledTimes(1);
  });

  it("keeps one token per plugin", async () => {
    const mint = vi.fn(async (id: string) => minted(id));
    expect((await hostSession("a@m", mint)).token).toBe("tok-a@m");
    expect((await hostSession("b@m", mint)).token).toBe("tok-b@m");
  });

  it("does not keep a failure, so a retry asks again", async () => {
    const mint = vi.fn().mockRejectedValueOnce(new Error("plugin not found")).mockResolvedValueOnce(minted("a@m"));
    await expect(hostSession("a@m", mint)).rejects.toThrow("plugin not found");
    expect(await hostSession("a@m", mint)).toEqual(minted("a@m"));
    expect(mint).toHaveBeenCalledTimes(2);
  });

  it("asks again after it was forgotten", async () => {
    const mint = vi.fn(async (id: string) => minted(id));
    await hostSession("a@m", mint);
    forgetHostSession("a@m");
    await hostSession("a@m", mint);
    expect(mint).toHaveBeenCalledTimes(2);
  });
});
