# cohost-dl
Downloads posts onto your computer from cohost.org, which is shutting down.

- Post pages are downloaded exactly as they appear on Cohost, including shared posts, comments, and with your display settings (silenced tags, etc.)
- Downloads all of your own posts and all of your liked posts
- If you have a data portability archive: also downloads all posts you’ve commented on
- Legal: using this software does not somehow grant you a license to re-publish posts and comments from other people

Related: [cohost-dl 2](db)

## Downloaded Data
Downloaded data will be placed in an `out` directory.

<details>
<summary>Detailed breakdown</summary>

- HTML files openable in a web browser
  - `out/index.html`: a simple overview page
  - `out/~all/index.html`: The Cohost Archive Global Feed
  - `out/{handle}/index.html`: page that shows all posts from {handle}
  - `out/{handle}/post/12345-example.html`: page that shows just that post, as it appeared on cohost.org
- Page resources
  - `out/static/`: files from cohost.org/static, such as CSS files
  - `out/rc/attachment/`: post images and audio files
  - `out/rc/attachment-redirect/`: honestly, no idea. ostensibly also post attachments
  - `out/rc/avatar/`, `out/rc/default-avatar/`: user avatars
  - `out/rc/header/`: user header images
  - `out/rc/external/`: external images not hosted on cohost.org but included in posts
  - `out/{handle}/cdl-index.js`: full-text search index
  - `out/{handle}/cdl-chunk~{handle}~{n}.js`: post data used in the list of all posts
  - `out/~cohost-dl/`: Javascript for all generated pages
- Data files
  - `out/{your-handle}/liked.json`: data for all posts you liked
  - `out/{your-handle}/posts.json`: data for all posts you made
  - `out/{handle}/post/12345-example` (without `.html`): original data for that post from cohost.org
  - `out/~src/{site-version}/`: unpacked source code for the Cohost frontend (used to create cohost-dl Javascript)
  - `out/~headers.json`: stores content type headers for some URLs that don’t have a good file extension

</details>

For file size, expect something around 1 GB for 1000 posts.

Files you can probably safely rehost online:
- `out/{your-handle}/index.html`
- `out/{your-handle}/cdl-index.js`
- `out/{your-handle}/cdl-chunk~{...}.js`
- `out/~cohost-dl/`
- files in `out/rc/` required for the above page(s) to work

Why other files may not be safe to rehost online:
- `out/{your-handle}/post/12345-example.html`: is a very faithful Cohost page and hence contains all of your settings (sideblogs, muted tags, etc.)
  - The `GENERIC_OBSERVER` setting attempts to mitigate this, but it breaks a bunch of other things
- `out/{not-your-handle}/`: not yours

## Usage
1. Copy `CONFIG.example.ts` to `CONFIG.ts`
2. edit `CONFIG.ts` appropriately
3. Install Deno
4. `./run.sh`
    - if you’re using a system that doesn’t support Bash, such as Windows,
      you can just copy the `deno run ...` command from this file and run it directly.

It's safe to interrupt and re-start the script at any time.
Things that have already been downloaded will not be downloaded again,
and any changes in configuration will be taken into account upon restart.

