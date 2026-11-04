create table agent_states (
  repo       text not null collate nocase,
  agent      text not null collate nocase,
  state      text not null,
  updated_at text not null,
  primary key (repo, agent)
);
