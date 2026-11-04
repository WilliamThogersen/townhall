create table conversations (
  id              text primary key,
  session_id      text not null unique,
  session_started integer not null default 0,
  title           text not null,
  status          text not null,
  linked_repo     text,
  ended           integer not null default 0,
  context_used    integer,
  context_limit   integer,
  last_entry_id   integer not null default 0,
  created_at      text not null,
  updated_at      text not null
);

create index conversations_by_activity on conversations (updated_at desc, created_at desc);

create table messages (
  conversation_id text not null references conversations (id) on delete cascade,
  id              integer not null,
  author          text not null check (author in ('you', 'townhall')),
  text            text not null,
  attachments     text not null default '[]',
  at              text not null,
  primary key (conversation_id, id)
);

create table events (
  conversation_id text not null references conversations (id) on delete cascade,
  id              integer not null,
  kind            text not null,
  data            text not null,
  at              text not null,
  primary key (conversation_id, id)
);

create table tasks (
  name            text primary key collate nocase,
  conversation_id text not null references conversations (id) on delete cascade,
  title           text not null,
  prompt          text not null,
  repo            text not null,
  base_branch     text not null,
  after           text not null default '[]',
  state           text not null,
  attachments     text not null default '[]',
  pr_number       integer,
  pr_url          text,
  pr_checks       text,
  created_at      text not null
);

create index tasks_by_conversation on tasks (conversation_id, created_at);
create index tasks_by_state on tasks (state);

create table attachments (
  id              text primary key,
  conversation_id text not null references conversations (id) on delete cascade,
  name            text not null,
  mime            text not null,
  size            integer not null,
  kind            text not null check (kind in ('image', 'file')),
  created_at      text not null
);

create index attachments_by_conversation on attachments (conversation_id, created_at);
