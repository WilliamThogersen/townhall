export const BY_REPO_KEY = "th-sidebar-by-repo";

export function loadByRepo(): boolean {
  try {
    return window.localStorage.getItem(BY_REPO_KEY) === "true";
  } catch {
    return false;
  }
}

export function saveByRepo(on: boolean): boolean {
  try {
    window.localStorage.setItem(BY_REPO_KEY, String(on));
    return true;
  } catch {
    return false;
  }
}
