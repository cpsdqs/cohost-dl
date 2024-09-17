# cohost-dl
Downloads posts onto your computer from cohost.org, which is shutting down.

- Post pages are downloaded exactly as they appear on Cohost, including shared posts, comments, and with your display settings (silenced tags, etc.)
- Downloads all of your own posts and all of your liked posts
- If you have a data portability archive: also downloads all posts you’ve commented on
- Legal: using this software does not somehow grant you a license to re-publish posts and comments from other people

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
