create table conversation_turns (
  id              integer primary key autoincrement,
  conversation_id text not null references conversations (id) on delete cascade,
  state           text not null,
  tokens          integer not null default 0,
  created_at      text not null,
  ended_at        text not null
);

create index conversation_turns_by_conversation on conversation_turns (conversation_id, created_at);
