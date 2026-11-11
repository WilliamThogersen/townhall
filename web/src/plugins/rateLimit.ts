export const FRAME_MESSAGES_PER_SECOND = 50;

export function createRateLimiter(
  limit: number = FRAME_MESSAGES_PER_SECOND,
  windowMs = 1000,
  now: () => number = Date.now,
): () => boolean {
  const seen: number[] = [];
  return () => {
    const at = now();
    while (seen.length > 0 && at - (seen[0] ?? at) >= windowMs) seen.shift();
    if (seen.length >= limit) return false;
    seen.push(at);
    return true;
  };
}
