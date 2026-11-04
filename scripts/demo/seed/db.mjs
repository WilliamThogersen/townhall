import { execFileSync } from "node:child_process";
import { join } from "node:path";

const BUSY_MS = 5000;

export const sqlText = (value) => (value === null || value === undefined ? "null" : `'${String(value).replaceAll("'", "''")}'`);

export function database(home) {
  const path = join(home, "townhall.db");
  return {
    path,
    run(statements) {
      if (statements.length === 0) return "";
      const script = [`.timeout ${BUSY_MS}`, "begin immediate;", ...statements, "commit;"].join("\n");
      return execFileSync("sqlite3", ["-bail", path], { input: script, stdio: ["pipe", "pipe", "pipe"] }).toString();
    },
    query(sql) {
      return execFileSync("sqlite3", ["-json", path], { input: `.timeout ${BUSY_MS}\n${sql}`, stdio: ["pipe", "pipe", "pipe"] }).toString();
    },
  };
}
