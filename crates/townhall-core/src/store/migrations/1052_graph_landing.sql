alter table graph_recorded add column landed integer not null default 0;

update graph_recorded set landed = 0;

create table graph_scan (
  repo text primary key collate nocase,
  head text not null
);
