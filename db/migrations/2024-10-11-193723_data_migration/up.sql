create table data_migration_state
(
    name  varchar not null primary key,
    value varchar not null
);

alter table posts
    add column is_adult_content boolean not null default false;
alter table posts
    add column is_pinned boolean not null default false;
