# Voice and microcopy

townhall is a control surface. Words should be short, literal and matter-of-fact: say what happened, what it affects, what to do. No cheerfulness, no apology, no civic cosplay (no "citizens", "assembly", "decree"). The building is in the icon, not the copy.

## Rules

1. **Literal.** Name the thing: "agent", "worktree", "branch", "session". Do not rename them ("worker", "run", "bot").
2. **Short.** One sentence where possible. Under 60 characters for toasts and banner leads.
3. **Facts first.** Lead with what happened or the current state, then the cause, then the action.
4. **Sentence case** everywhere: buttons, titles, tabs, labels ("Create agent", not "Create Agent"). Status labels in the UI render uppercase via CSS from sentence-case source text.
5. **Verb plus object on buttons.** "Create agent", "Stop and remove", "Change repo", not "Submit", "OK", "Yes".
6. **Present participle plus ellipsis for in-progress.** "Creating...", "Sending...", "Stopping...". Three ASCII dots, no ellipsis character.
7. **No exclamation marks, no emoji, no "please", no "sorry", no "Oops".**
8. **Agent names, branches, paths and commands** are always in mono and never reworded.
9. **Numbers as digits**, units spelled short ("6 min", "2 s"). Plurals handled: "1 agent", "12 agents", never "agent(s)".
10. **No dashes as punctuation** in copy (use a period, comma or colon).
11. **Do not blame the user.** "Name must use lowercase letters, numbers and hyphens", not "You entered an invalid name".

## Errors

Pattern: `<What failed>: <reason>.` plus the next step only if one exists. Keep the system's reason text verbatim when it is useful.

| Bad | Good |
|-----|------|
| Oops! Something went wrong. | Cannot refresh agents: connection refused. |
| Invalid input | Name is required. |
| Failed | Could not create worktree: branch already exists. |
| An error occurred while stopping the agent, please try again. | Could not stop fix-login: tmux session not found. |

- Field errors say what is needed, not what was wrong: "Name is required.", "Use lowercase letters, numbers and hyphens.", "Command is required for shell agents."
- Retrying errors say so: "Cannot refresh agents: connection refused. Retrying."
- Destructive failure: say what state things are in: "Could not stop fix-login. The agent is still running."
- Never show raw stack traces or HTTP bodies in the UI; put them behind a "Details" disclosure.

## Empty states

Title states the fact; body says what the thing is or the next step; at most one button.

| Place | Title | Body | Button |
|-------|-------|------|--------|
| Agents, none | No agents yet | An agent is one task in its own worktree. Create one to see it here. | Create agent |
| Activity, none | Nothing has happened yet | Create an agent and its activity shows up here. | Create agent |
| Filter, no match | No events match | Try another filter. | Clear filters |
| No repo (desktop) | Choose a repository | Choose the git repository your agents will work in. | Choose repository... |

## Labels and buttons

| Context | Copy |
|---------|------|
| Create form | Name, Adapter, Prompt (optional), Command, Base (optional); button "Create agent" |
| Placeholders | `fix-login`, `npm test --watch`, `HEAD`, "Send input to the agent" |
| Card actions | Open / Close, Stop |
| Stop dialog | Title "Stop fix-login?"; body "The agent ends and its worktree is deleted. Uncommitted work is lost."; buttons "Cancel", "Stop and remove" |
| Send box | "Send", "Sending..." |
| Repo | "Change repo", "Choose repository..." (trailing dots mean a picker opens) |
| Stream status | Live, Stream connecting..., Stream closed, Session ended: REASON |
| Tabs | Activity, Agents |

## Status and feed wording

| State | Label | Feed verb and detail |
|-------|-------|----------------------|
| alive | Alive | Finished: "Task complete in 6 min" |
| working | Working | Started: "claude: Refactor session handling" |
| waiting | Waiting | Needs input: the question itself, verbatim |
| dead | Dead | Died: "Session ended, exit code 1" |
| error | Error | Error: "Could not create worktree: branch exists" |

## Toasts and notifications

Title is the subject and the event: "fix-login needs input", "docs-sweep died", "Agent created". Detail is one fact: "Waiting for a reply for 2 min", "Session ended, exit code 1", "refactor-auth is running". OS notifications reuse the same two strings.

## Accessibility copy

- Icon buttons: label is the action and object ("Stop fix-login", "Dismiss notification", "Close terminal for fix-login").
- Live regions announce the toast title and detail, not decorative text.
- Do not put meaning only in an icon or color; every status glyph sits next to its word.
