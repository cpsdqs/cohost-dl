create table resource_content_types
(
    url          varchar not null primary key,
    content_type varchar not null
);

create table url_files
(
    url       varchar not null primary key,
    file_path blob    not null
);

create table projects
(
    id                 integer not null,
    handle             varchar not null collate nocase,
    is_private         boolean not null,
    requires_logged_in boolean not null,
    data               blob    not null,
    data_version       integer not null,
    primary key (id)
);

create index projects_handle on projects (handle);

create table project_resources
(
    project_id integer not null,
    url        varchar not null,
    primary key (project_id, url) on conflict ignore,
    foreign key (project_id) references projects (id) on delete cascade
);

create table posts
(
    id                   integer not null,
    posting_project_id   integer not null,
    published_at         varchar,
    response_to_ask_id   varchar,
    share_of_post_id     integer,
    is_transparent_share boolean not null,
    filename             varchar not null,
    data                 blob    not null,
    data_version         integer not null,
    state                integer not null,
    primary key (id) on conflict replace,
    foreign key (posting_project_id) references projects (id) on delete restrict,
    foreign key (share_of_post_id) references posts (id) on delete cascade
);

create index posts_posting_project_id on posts (posting_project_id);
create index posts_published_at on posts (published_at);

create table post_related_projects
(
    post_id    integer not null,
    project_id integer not null,
    primary key (post_id, project_id) on conflict ignore,
    foreign key (post_id) references posts (id) on delete cascade,
    foreign key (project_id) references projects (id) on delete cascade
);

create table post_resources
(
    post_id integer not null,
    url     varchar not null,
    primary key (post_id, url) on conflict ignore,
    foreign key (post_id) references posts (id) on delete cascade
);

create table post_tags
(
    post_id integer not null,
    tag     varchar not null,
    pos     integer not null,
    primary key (post_id, tag),
    foreign key (post_id) references posts (id) on delete cascade
);

create index post_tags_pos on post_tags (pos);

create table comments
(
    id                 varchar not null primary key on conflict replace,
    post_id            integer not null,
    in_reply_to_id     varchar,
    posting_project_id integer,
    published_at       varchar not null,
    data               blob    not null,
    data_version       integer not null,
    foreign key (post_id) references posts (id) on delete cascade,
    foreign key (posting_project_id) references projects (id) on delete cascade
);

create index comments_posting_project_id on comments (posting_project_id);
create index comments_post_id on comments (post_id);
create index comments_published_at on comments (published_at);

create table comment_resources
(
    comment_id varchar not null,
    url        varchar not null,
    primary key (comment_id, url) on conflict ignore,
    foreign key (comment_id) references comments (id) on delete cascade
);

create table likes
(
    from_project_id integer not null,
    to_post_id      integer not null,
    primary key (from_project_id, to_post_id) on conflict ignore,
    foreign key (to_post_id) references posts (id) on delete no action,
    foreign key (from_project_id) references projects (id) on delete restrict
);

create table follows
(
    from_project_id integer not null,
    to_project_id   integer not null,
    primary key (from_project_id, to_project_id) on conflict ignore,
    foreign key (from_project_id) references projects (id) on delete cascade,
    foreign key (to_project_id) references projects (id) on delete cascade
);
