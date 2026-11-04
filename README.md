# castle

An AI agent orchestrator. Rust core, React frontend. Built to run 10-50 agents at once and keep them autonomous.

Status: early scaffold.

## Goals

- Spawn, supervise and observe many agents in parallel
- Git worktree per agent
- Adapter trait so any agent CLI can plug in
- Live terminal streaming to a React UI
- Custom behavior lives in plugins, not in core
