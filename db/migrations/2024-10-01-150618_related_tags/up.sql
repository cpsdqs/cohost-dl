create table related_tags
(
    tag1       varchar not null collate nocase,
    tag2       varchar not null collate nocase,
    is_synonym integer not null,
    primary key (tag1, tag2),
    constraint ordering check (tag1 < tag2)
);

create index related_tags_is_synonym on related_tags (is_synonym);
