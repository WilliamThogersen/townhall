update agents
set source = replace(source,
      'a task that is quiet is still working.',
      'a task that is quiet is still working. when an event says all work of this conversation has landed, post the closing summary it asks for (what landed, PR numbers, merge shas) in this thread; never stay silent on it.'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and instr(source, 'a task that is quiet is still working.') > 0
  and instr(source, 'post the closing summary it asks for') = 0;
