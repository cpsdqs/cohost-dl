# cohost-dl
Archives data from cohost.org, which is shutting down.

## Usage
1. `cp CONFIG.example.ts CONFIG.ts`
2. edit `CONFIG.ts` appropriately
3. Install Deno
4. `./run.sh`
    - if you’re using a system that doesn’t support bash,
      you can probably just copy the `deno run ...` command from this file and run it directly.

If it breaks, you can restart it, and it will try and pick up where it left off.
