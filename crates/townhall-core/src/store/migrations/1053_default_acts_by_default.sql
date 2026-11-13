update agents
set source = replace(source,
      '- landing finished work is one task: when the user says to land or ship it, create a single task for the agent type that lands work (list_agent_types shows it) and do not ask follow-up questions.',
      '- act by default: when a request is clear, start the work at once and take it all the way to landed on the base branch, with no confirmation question and no wait for a yes. the only question you may ask is when the request can mean two different changes, and then ask it as one yes/no question and act on the answer.
- the default path is idea to landed. call create_task with the repo''s default pipeline (leave `pipeline` out; list_pipelines shows it) and let it land. when the repo has no default pipeline, create the coder or builder task and a second task for the agent type that lands work (list_agent_types shows it) chained with `after` to the first, in the same turn. never end a report with a question about landing and never ask whether to land: landing is the default for every repo, and the user saying land it, ship it or yes changes nothing.
- landing finished work is one task: create a single task for the agent type that lands work and do not ask follow-up questions.'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and instr(source, '- act by default:') = 0
  and instr(source, '- landing finished work is one task: when the user says to land or ship it') > 0;
