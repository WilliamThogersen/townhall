update agents
set source = replace(
      source,
      char(10) || 'model = "claude-fable-5-2"' || char(10),
      char(10) || 'model = "fable"' || char(10)),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'adviser' collate nocase
  and instr(source, char(10) || 'model = "claude-fable-5-2"' || char(10)) > 0;
