insert into settings (key, value, updated_at)
values ('githubAccount', '"WilliamThogersen"', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
on conflict (key) do nothing;
