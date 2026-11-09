alter table tasks add column done_when text;
alter table tasks add column check_cmd text;
alter table tasks add column max_retries integer not null default 2;
alter table tasks add column verify text;
