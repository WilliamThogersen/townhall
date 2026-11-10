create table plugin_settings (
  plugin_id  text not null collate nocase,
  key        text not null,
  value      text not null,
  updated_at text not null,
  primary key (plugin_id, key)
);
