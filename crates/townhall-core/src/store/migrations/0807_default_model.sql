update agents
set source = 'model = "opus"' || char(10) || source,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
where name = 'default' collate nocase
  and (char(10) || source) not like '%' || char(10) || 'model%';
