create table adviser_calls (
  id integer primary key autoincrement,
  conversation_id text not null,
  task text,
  agent text not null,
  repo text not null default '',
  question text not null,
  answer text,
  state text not null,
  model text,
  tokens integer not null default 0,
  created_at text not null,
  ended_at text not null
);
create index adviser_calls_agent on adviser_calls (agent collate nocase, created_at);
