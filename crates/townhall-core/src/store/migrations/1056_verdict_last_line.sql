update agents
set source = replace(source,
      'prompt = """
',
      'prompt = """
the last line of your final message must be exactly VERDICT: PASS or VERDICT: FAIL, nothing after it.
'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'tester' collate nocase
  and instr(source, 'VERDICT: PASS') = 0
  and instr(source, 'prompt = """
') > 0;

update agents
set source = replace(source,
      'prompt = """
',
      'prompt = """
the last line of your final message must be exactly VERDICT: APPROVE or VERDICT: REJECT, nothing after it.
'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'reviewer' collate nocase
  and instr(source, 'VERDICT: APPROVE') = 0
  and instr(source, 'prompt = """
') > 0;
