create table agents (
  name        text primary key collate nocase,
  description text not null default '',
  source      text not null,
  updated_at  text not null
);
