create table turn_resumes (
  conversation_id text primary key references conversations (id) on delete cascade,
  entry_id        integer not null
);
