create table tracked_prs (
  repo         text not null,
  number       integer not null,
  url          text,
  branch       text not null,
  base         text not null,
  owner        text not null,
  pusher       text,
  head         text,
  status       text not null,
  detail       text,
  reruns       text not null default '[]',
  woke_for     text,
  seen_pending integer not null default 0,
  updated_at   text not null,
  primary key (repo, number)
);

create index tracked_prs_owner on tracked_prs (owner);
create index tracked_prs_pusher on tracked_prs (pusher);
