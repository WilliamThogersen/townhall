create table workflows (
  name        text primary key collate nocase,
  description text not null,
  source      text not null,
  updated_at  text not null
);

create table workflow_asks (
  conversation_id text not null references conversations (id) on delete cascade,
  name            text not null collate nocase,
  source          text not null,
  entry_id        integer not null,
  asked_at        text not null,
  primary key (conversation_id, name)
);

create table runs (
  id              text primary key,
  workflow        text not null,
  snapshot        text not null,
  conversation_id text not null references conversations (id) on delete cascade,
  repo            text not null,
  base_branch     text not null,
  state           text not null default 'running',
  created_at      text not null,
  ended_at        text
);

create index runs_by_conversation on runs (conversation_id, created_at);

create table run_steps (
  run_id  text not null references runs (id) on delete cascade,
  step_id text not null,
  round   integer not null default 1,
  task    text,
  state   text not null,
  verdict text,
  note    text,
  primary key (run_id, step_id, round)
);
