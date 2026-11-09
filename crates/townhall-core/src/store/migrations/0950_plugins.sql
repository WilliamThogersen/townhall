create table marketplaces (
  name       text primary key collate nocase,
  source     text not null,
  account    text,
  enabled    integer not null default 1,
  official   integer not null default 0,
  head_sha   text,
  fetched_at text,
  error      text
);

create table plugins (
  id               text primary key collate nocase,
  marketplace      text not null collate nocase references marketplaces (name),
  name             text not null collate nocase,
  version          text,
  sha              text not null,
  source           text not null,
  enabled          integer not null default 0,
  pinned           integer not null default 0,
  permissions_hash text not null,
  approved         text not null,
  installed_at     text not null,
  updated_at       text not null,
  error            text
);

create index plugins_by_marketplace on plugins (marketplace);
