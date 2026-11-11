update agents
set source = replace(source,
      '  "mcp__townhall__save_agent_type",
',
      '  "mcp__townhall__save_agent_type",
  "mcp__townhall__list_pipelines",
  "mcp__townhall__read_pipeline",
  "mcp__townhall__save_pipeline",
'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and instr(source, 'list_pipelines') = 0
  and instr(source, 'read_pipeline') = 0
  and instr(source, 'save_pipeline') = 0
  and instr(source, '  "mcp__townhall__save_agent_type",
') > 0;
