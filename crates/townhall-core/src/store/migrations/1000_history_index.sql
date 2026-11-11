create virtual table history_fts using fts5(
  text,
  kind unindexed,
  conversation_id unindexed,
  src_id unindexed,
  task unindexed,
  at unindexed,
  tokenize = 'porter unicode61'
);

insert into history_fts (text, kind, conversation_id, src_id, task, at)
select text, 'message', conversation_id, cast(id as text), null, at from messages;

insert into history_fts (text, kind, conversation_id, src_id, task, at)
select title || char(10) || prompt, 'task', conversation_id, name, name, created_at from tasks;

insert into history_fts (text, kind, conversation_id, src_id, task, at)
select json_extract(data, '$.question'), 'needs_you', conversation_id, cast(id as text),
       json_extract(data, '$.agent'), at
from events where kind = 'needsYou';

insert into history_fts (text, kind, conversation_id, src_id, task, at)
select json_extract(data, '$.text'), 'agent_report', conversation_id, cast(id as text),
       substr(json_extract(data, '$.text'), 6, instr(substr(json_extract(data, '$.text'), 6), ' ') - 1), at
from events
where kind = 'event'
  and (json_extract(data, '$.text') like 'task % ended (%'
       or json_extract(data, '$.text') like 'task % failed: %');

create trigger history_messages_insert after insert on messages begin
  insert into history_fts (text, kind, conversation_id, src_id, task, at)
  values (new.text, 'message', new.conversation_id, cast(new.id as text), null, new.at);
end;

create trigger history_messages_delete after delete on messages begin
  delete from history_fts
  where kind = 'message' and conversation_id = old.conversation_id and src_id = cast(old.id as text);
end;

create trigger history_messages_update after update on messages begin
  delete from history_fts
  where kind = 'message' and conversation_id = old.conversation_id and src_id = cast(old.id as text);
  insert into history_fts (text, kind, conversation_id, src_id, task, at)
  values (new.text, 'message', new.conversation_id, cast(new.id as text), null, new.at);
end;

create trigger history_tasks_insert after insert on tasks begin
  insert into history_fts (text, kind, conversation_id, src_id, task, at)
  values (new.title || char(10) || new.prompt, 'task', new.conversation_id, new.name, new.name, new.created_at);
end;

create trigger history_tasks_delete after delete on tasks begin
  delete from history_fts where kind = 'task' and src_id = old.name;
end;

create trigger history_tasks_update after update of name, title, prompt, conversation_id on tasks
when old.name is not new.name or old.title is not new.title or old.prompt is not new.prompt
  or old.conversation_id is not new.conversation_id
begin
  delete from history_fts where kind = 'task' and src_id = old.name;
  insert into history_fts (text, kind, conversation_id, src_id, task, at)
  values (new.title || char(10) || new.prompt, 'task', new.conversation_id, new.name, new.name, new.created_at);
end;

create trigger history_events_insert after insert on events begin
  insert into history_fts (text, kind, conversation_id, src_id, task, at)
  select json_extract(new.data, '$.question'), 'needs_you', new.conversation_id, cast(new.id as text),
         json_extract(new.data, '$.agent'), new.at
  where new.kind = 'needsYou';
  insert into history_fts (text, kind, conversation_id, src_id, task, at)
  select json_extract(new.data, '$.text'), 'agent_report', new.conversation_id, cast(new.id as text),
         substr(json_extract(new.data, '$.text'), 6, instr(substr(json_extract(new.data, '$.text'), 6), ' ') - 1), new.at
  where new.kind = 'event'
    and (json_extract(new.data, '$.text') like 'task % ended (%'
         or json_extract(new.data, '$.text') like 'task % failed: %');
end;

create trigger history_events_delete after delete on events begin
  delete from history_fts
  where kind in ('needs_you', 'agent_report')
    and conversation_id = old.conversation_id and src_id = cast(old.id as text);
end;

create trigger history_events_update after update on events begin
  delete from history_fts
  where kind in ('needs_you', 'agent_report')
    and conversation_id = old.conversation_id and src_id = cast(old.id as text);
  insert into history_fts (text, kind, conversation_id, src_id, task, at)
  select json_extract(new.data, '$.question'), 'needs_you', new.conversation_id, cast(new.id as text),
         json_extract(new.data, '$.agent'), new.at
  where new.kind = 'needsYou';
  insert into history_fts (text, kind, conversation_id, src_id, task, at)
  select json_extract(new.data, '$.text'), 'agent_report', new.conversation_id, cast(new.id as text),
         substr(json_extract(new.data, '$.text'), 6, instr(substr(json_extract(new.data, '$.text'), 6), ' ') - 1), new.at
  where new.kind = 'event'
    and (json_extract(new.data, '$.text') like 'task % ended (%'
         or json_extract(new.data, '$.text') like 'task % failed: %');
end;
