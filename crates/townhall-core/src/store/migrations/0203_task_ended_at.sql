alter table tasks add column ended_at text;
update tasks set ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') where state = 'ended';
