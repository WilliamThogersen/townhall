create table graph_edges (
  src_kind text not null,
  src      text not null collate nocase,
  rel      text not null,
  dst_kind text not null,
  dst      text not null collate nocase,
  at       text not null,
  primary key (src_kind, src, dst_kind, dst)
);

create index graph_edges_by_dst on graph_edges (dst_kind, dst);

create table graph_recorded (
  task text primary key collate nocase,
  at   text not null
);

create trigger graph_tasks_delete after delete on tasks begin
  delete from graph_edges
  where (src_kind = 'task' and src = old.name) or (dst_kind = 'task' and dst = old.name);
  delete from graph_recorded where task = old.name;
end;
