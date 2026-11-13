update agents
set source = replace(source,
      '- act by default: when a request is clear, start the work at once and take it all the way to landed on the base branch, with no confirmation question and no wait for a yes. the only question you may ask is when the request can mean two different changes, and then ask it as one yes/no question and act on the answer.',
      '- act by default: when a request is clear, start the work at once and take it all the way to landed on the base branch, with no confirmation question and no wait for a yes. the only question you may ask before starting work is when the request can mean two different changes, and then ask it as one yes/no question and act on the answer. this does not cover designing an agent type with the user or a save that townhall asks the user to confirm: follow the rules above for those.'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and instr(source, 'this does not cover designing an agent type') = 0
  and instr(source, 'the only question you may ask is when the request can mean two different changes') > 0;

update agents
set source = replace(source,
      '- every task names an agent type, the saved definition its agent runs as (model, tools, permission mode, personality prompt). pass it as `agent` on create_task; left out, the task runs as builder. list_agent_types',
      '- every task names an agent type, the saved definition its agent runs as (model, tools, permission mode, personality prompt). pass it as `agent` on create_task; left out, the task runs as builder, or starts the repo''s default pipeline when there is one. list_agent_types'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and instr(source, 'or starts the repo''s default pipeline when there is one') = 0
  and instr(source, 'pass it as `agent` on create_task; left out, the task runs as builder. list_agent_types') > 0;

update agents
set source = replace(source,
      '- the default path is idea to landed. call create_task with the repo''s default pipeline (leave `pipeline` out; list_pipelines shows it) and let it land. when the repo has no default pipeline, create the coder or builder task and a second task for the agent type that lands work (list_agent_types shows it) chained with `after` to the first, in the same turn. never end a report with a question about landing and never ask whether to land: landing is the default for every repo, and the user saying land it, ship it or yes changes nothing.',
      '- the default path is idea to landed. to start the repo''s default pipeline (list_pipelines shows it), call create_task with `pipeline` set to it and `agent`, `after`, `prBase`, `check`, `doneWhen`, `maxRetries` and `attachments` all left out, and let it land: setting any of those creates one plain task that never lands. when you must set one of those, or the repo has no default pipeline, create the coder or builder task and a second task for the agent type that lands work (list_agent_types shows it) chained with `after` to the first, in the same turn. never end a report with a question about landing and never ask whether to land: landing is the default for every repo and needs no go-ahead, so do not wait for land it, ship it or yes before landing.'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and instr(source, 'do not wait for land it, ship it or yes before landing') = 0
  and instr(source, 'call create_task with the repo''s default pipeline (leave `pipeline` out; list_pipelines shows it) and let it land.') > 0;
