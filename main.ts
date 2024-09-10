#!/usr/bin/env deno --allow-net --allow-read=out --allow-write=out

import { COOKIE, PROJECTS, POSTS } from './CONFIG.ts';
import { CohostContext } from './context.ts';
import { loadAllLikedPosts } from "./likes.ts";
import { filePathForPost, loadPostPage } from "./post-page.ts";
import { loadAllProjectPosts } from "./project.ts";
import { IPost } from "./model.ts";

const ctx = new CohostContext(COOKIE, 'out');

if (!(await ctx.hasFile('liked.json'))) {
    const liked = await loadAllLikedPosts(ctx);
    await ctx.write('liked.json', JSON.stringify(liked));
}

for (const handle of PROJECTS) {
    if (!(await ctx.hasFile(`${handle}/posts.json`))) {
        const posts = await loadAllProjectPosts(ctx, handle);
        await ctx.write(`${handle}/posts.json`, JSON.stringify(posts));
    }
}

const likedPosts= await ctx.readJson('liked.json') as IPost[];
const projectPosts = await Promise.all(PROJECTS.map(handle => ctx.readJson(`${handle}/posts.json`))) as IPost[];

for (const post of [...likedPosts, ...projectPosts]) {
    const filePath = filePathForPost(post);
    if (!(await ctx.hasFile(filePath))) {
        console.log(`~~ loading post ${post.singlePostPageUrl}`);
        await loadPostPage(ctx, post.singlePostPageUrl);
    }
}

for (const post of POSTS) {
    console.log(`~~ loading additional post ${post}`);
    await loadPostPage(ctx, post);
}
