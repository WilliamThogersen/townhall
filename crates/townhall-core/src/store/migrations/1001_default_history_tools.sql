update agents
set source = replace(source,
      '  "mcp__townhall__conversation_insights",
',
      '  "mcp__townhall__conversation_insights",
  "mcp__townhall__search_history",
  "mcp__townhall__read_conversation",
'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and instr(source, 'search_history') = 0
  and instr(source, '  "mcp__townhall__conversation_insights",
') > 0;
update agents
set source = replace(source,
      '- landing finished work is one task',
      '- before planning work that may have been done or rejected before, call search_history to find it in earlier conversations and read_conversation to open one.
- landing finished work is one task'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and instr(source, 'mcp__townhall__search_history') > 0
  and instr(source, 'call search_history') = 0
  and instr(source, '- landing finished work is one task') > 0;
