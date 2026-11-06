create table artifacts (
  id              text primary key,
  task            text not null collate nocase references tasks (name) on delete cascade,
  conversation_id text not null,
  name            text not null,
  kind            text not null,
  mime            text not null,
  size            integer not null,
  author_role     text not null,
  caption         text,
  created_at      text not null
);

create index artifacts_by_task on artifacts (task, created_at);
