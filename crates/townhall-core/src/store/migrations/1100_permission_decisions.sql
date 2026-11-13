create table permission_decisions (
  id integer primary key autoincrement,
  task text not null,
  agent_type text not null collate nocase,
  command text not null,
  pattern text not null default '',
  verdict text not null,
  reason text not null default '',
  created_at text not null
);
create index permission_decisions_pattern on permission_decisions (agent_type, pattern, verdict);
