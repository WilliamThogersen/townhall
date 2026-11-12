update agents
set source = replace(source,
      '  "mcp__townhall__read_conversation",
',
      '  "mcp__townhall__read_conversation",
  "mcp__townhall__history_graph",
  "mcp__townhall__agent_stats",
'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and instr(source, 'history_graph') = 0
  and instr(source, '  "mcp__townhall__read_conversation",
') > 0;
update agents
set source = replace(source,
      '- landing finished work is one task',
      '- before touching an area that has a history, call history_graph with its file path or folder glob to see the tasks and conversations that touched it, who did them and how they ended. when asked how agents perform, call agent_stats.
- landing finished work is one task'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and instr(source, 'mcp__townhall__history_graph') > 0
  and instr(source, 'call history_graph') = 0
  and instr(source, '- landing finished work is one task') > 0;
