create table task_handoffs (
  task      text not null collate nocase references tasks (name) on delete cascade,
  seq       integer not null,
  from_type text not null,
  to_type   text not null,
  reason    text not null,
  summary   text not null,
  at        text not null,
  primary key (task, seq)
);
