create table plugin_storage (
  plugin_id  text not null collate nocase,
  key        text not null,
  value      text not null,
  updated_at text not null,
  primary key (plugin_id, key)
);

create table plugin_tokens (
  token_hash text primary key,
  plugin_id  text not null collate nocase,
  task       text collate nocase,
  created_at text not null,
  revoked_at text
);

create index plugin_tokens_by_plugin on plugin_tokens (plugin_id);
create index plugin_tokens_by_task on plugin_tokens (task);
