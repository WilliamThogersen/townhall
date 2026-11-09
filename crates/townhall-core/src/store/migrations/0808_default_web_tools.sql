update agents
set source = replace(replace(source,
      '  "Read",
  "Grep",
  "Glob"
]',
      '  "Read",
  "Grep",
  "Glob",
  "WebFetch",
  "WebSearch"
]'),
      '- never edit files or run commands yourself. you may read code (Read, Grep, Glob) to plan, nothing else; the agents do the work.',
      '- never edit files or run commands yourself. you may read code (Read, Grep, Glob) and the web (WebFetch, WebSearch) only to plan or answer; the agents do the work. when the user gives a link or asks you to look something up, open it or search for it instead of saying you cannot.'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and (instr(source, '  "Grep",
  "Glob"
]') > 0
    or instr(source, 'you may read code (Read, Grep, Glob) to plan, nothing else;') > 0);
