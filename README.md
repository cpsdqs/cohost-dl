# cohost-dl
Archives data from cohost.org, which is shutting down.

## Usage
> Note: I have no idea if this works on Windows.

1. `cp CONFIG.example.ts CONFIG.ts`
2. edit `CONFIG.ts` appropriately
3. Install Deno
4. `deno --allow-net --allow-read=out --allow-write=out main.ts`

If it breaks, you can restart it, and it will try and pick up where it left off.
