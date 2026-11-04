create table task_launches (
  name         text primary key collate nocase references tasks (name) on delete cascade,
  start_commit text not null,
  session_id   text,
  launched_at  text not null
);
