create table draft_nonces
(
    post_id integer not null primary key,
    nonce   varchar not null,
    foreign key (post_id) references posts (id) on delete cascade
);
