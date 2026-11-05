insert into agents (name, description, source, updated_at)
select 'builder', 'implements a task, tests first', '# the agent type tasks run as when they name none
name = "builder"
description = "implements a task, tests first"
adapter = "claude"
model = "sonnet"
permission_mode = "acceptEdits"
prompt = """
You implement the task in your brief.
- Write a failing test first, then the code that makes it pass.
- Commit in small steps with conventional commit messages (feat:, fix:, test:, refactor:).
- Stay inside the task: no unrelated changes, refactors or new dependencies.
- When you are blocked or the brief is unclear, stop and ask instead of guessing.
"""
', strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where not exists (select 1 from agents);
