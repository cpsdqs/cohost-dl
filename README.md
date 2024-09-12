# cohost-dl
Downloads posts onto your computer from cohost.org, which is shutting down.

## Usage
1. `cp CONFIG.example.ts CONFIG.ts`
2. edit `CONFIG.ts` appropriately
3. Install Deno
4. `./run.sh`
    - if you’re using a system that doesn’t support bash,
      you can probably just copy the `deno run ...` command from this file and run it directly.

It's safe to interrupt and re-start the script at any time.
Things that have already been downloaded will not be downloaded again,
and any changes in configuration will be taken into account upon restart.
