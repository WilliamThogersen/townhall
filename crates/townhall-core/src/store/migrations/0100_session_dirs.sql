create table session_dirs (
  conversation_id text primary key references conversations (id) on delete cascade,
  cwd             text not null,
  updated_at      text not null
);
