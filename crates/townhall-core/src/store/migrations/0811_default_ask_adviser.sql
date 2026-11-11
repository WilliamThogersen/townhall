update agents
set source = replace(source,
      '  "mcp__townhall__conversation_insights",
',
      '  "mcp__townhall__conversation_insights",
  "mcp__townhall__ask_adviser",
'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and instr(source, 'ask_adviser') = 0
  and instr(source, '  "mcp__townhall__conversation_insights",
') > 0;
