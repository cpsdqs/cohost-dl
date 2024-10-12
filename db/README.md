# cohost-dl 2: DL harder
cohost-dl but downloading a lot more data and with even less usability

Notes:
- You can interrupt this at any time, but if it’s doing something where there’s no progress bar, it’ll start over from page 1.
  This is probably annoying if you were on, like, page 200.
- I am not very good at SQL

Download stages:
1. downloading posts
2. downloading post comments
3. downloading image and audio resources

Files:
- the database: stores all post data
- the output directory: stores all resources like images
- downloader-state.json: file to remember what’s already been downloaded before and skip downloading those things (can be edited)

> Note: if you have used cohost-dl 2 before, you should probably run it again with the `try_fix_transparent_shares` option.

## Compiling and running from source
1. compile the post & markdown renderer. this is super jank. it currently requires running cohost-dl 1 as well
    - if ASSC ever ships an open source post renderer, this will be replaced with that
    - if you don’t care about serve mode, just make an empty `md-render/compiled.js` file so the Rust code compiles
    - in repo root:
    - `rm out/staff/post/7611443-cohost-to-shut-down` (if it exists)
        - why? because this post is used to determine the current Cohost version
    - `./run.sh`
      - wait for it to download Cohost version `a2ecdc59`
      - if this is no longer the current Cohost version, then the following build script will need an update
    - `cd db/md-render`
    - `./build.sh`
2. `cargo run -- download`
3. `cargo run -- serve` (can be run in parallel)
