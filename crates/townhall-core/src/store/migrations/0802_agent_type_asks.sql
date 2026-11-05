create table agent_type_asks (
  conversation_id text not null references conversations (id) on delete cascade,
  name            text not null collate nocase,
  source          text not null,
  entry_id        integer not null,
  asked_at        text not null,
  primary key (conversation_id, name)
);
