drop table data_migration_state;

alter table posts drop column is_adult_content;
alter table posts drop column is_pinned;
