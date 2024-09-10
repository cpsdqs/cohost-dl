import { COOKIE, POSTS, PROJECTS, SKIP_POSTS, DATA_PORTABILITY_ARCHIVE_PATH } from "./CONFIG.ts";
import { CohostContext } from "./context.ts";
import { loadAllLikedPosts } from "./likes.ts";
import { filePathForPost, loadPostPage } from "./post-page.ts";
import { loadAllProjectPosts } from "./project.ts";
import { IPost } from "./model.ts";
import { readDataPortabilityArchiveItems } from "./data-portability-archive.ts";

const ctx = new CohostContext(COOKIE, "out");

{
    // check that login actually worked
    const loginStateResponse = await ctx.get(
        "https://cohost.org/api/v1/trpc/login.loggedIn?batch=1&input=%7B%7D",
    );
    const loginState = await loginStateResponse.json();
    if (!loginState[0].result.data.loggedIn) {
        throw new Error(
            "Not logged in. Please update your cookie configuration",
        );
    } else {
        console.log(`logged in as ${loginState[0].result.data.email}`);
    }
}

// JSON data
{
    // load all liked posts for the current page
    if (!(await ctx.hasFile("liked.json"))) {
        const liked = await loadAllLikedPosts(ctx);
        await ctx.write("liked.json", JSON.stringify(liked));
    }

    // load all project posts
    for (const handle of PROJECTS) {
        if (!(await ctx.hasFile(`${handle}/posts.json`))) {
            const posts = await loadAllProjectPosts(ctx, handle);
            await ctx.write(`${handle}/posts.json`, JSON.stringify(posts));
        }
    }
}

// Single post pages
{
    const likedPosts = await ctx.readJson("liked.json") as IPost[];
    const projectPosts = await Promise.all(
        PROJECTS.map((handle) => ctx.readJson(`${handle}/posts.json`)),
    ) as IPost[];

    for (const post of [...likedPosts, ...projectPosts]) {
        if (SKIP_POSTS.includes(post.postId)) continue;

        const filePath = filePathForPost(post);
        if (!(await ctx.hasFile(filePath))) {
            console.log(`~~ loading post ${post.singlePostPageUrl}`);
            await loadPostPage(ctx, post.singlePostPageUrl);
        }
    }

    const dpaPostURLs: string[] = [];
    if (DATA_PORTABILITY_ARCHIVE_PATH) {
        const items = await readDataPortabilityArchiveItems(DATA_PORTABILITY_ARCHIVE_PATH);
        for (const ask of items.asks) {
            if (ask.responsePost) {
                dpaPostURLs.push(ask.responsePost);
            }
        }
        for (const comment of items.comments) {
            if (comment.post) {
                dpaPostURLs.push(comment.post);
            } else {
                console.log(`comment ${comment.commentId} has no post`);
            }
        }
    }

    for (const post of [...POSTS, ...dpaPostURLs]) {
        if (await ctx.probablyHasFileForPostURL(post)) continue;

        console.log(`~~ loading additional post ${post}`);
        await loadPostPage(ctx, post);
    }
}

console.log('Done');
