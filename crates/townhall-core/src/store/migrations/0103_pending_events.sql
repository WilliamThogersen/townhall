create table pending_events (
  conversation_id text not null references conversations (id) on delete cascade,
  seq             integer not null,
  text            text not null,
  primary key (conversation_id, seq)
);
