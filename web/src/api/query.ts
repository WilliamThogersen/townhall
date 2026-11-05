export type QueryParams = Readonly<Record<string, string | null | undefined>>;

export function withQuery(path: string, params: QueryParams): string {
  const pairs = Object.entries(params).flatMap(([key, value]): [string, string][] =>
    value ? [[key, value]] : [],
  );
  const query = new URLSearchParams(pairs).toString();
  return query ? `${path}?${query}` : path;
}
