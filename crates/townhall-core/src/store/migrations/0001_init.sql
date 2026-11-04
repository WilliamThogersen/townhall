create table settings (
  key        text primary key,
  value      text not null,
  updated_at text not null
);

create table repos (
  name        text primary key,
  path        text not null unique,
  base_branch text not null,
  added_at    text not null
);

create unique index repos_name_nocase on repos (name collate nocase);
