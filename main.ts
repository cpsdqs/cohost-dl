import {
    COOKIE,
    DATA_PORTABILITY_ARCHIVE_PATH,
    ENABLE_JAVASCRIPT,
    POSTS,
    PROJECTS,
    SKIP_POSTS,
} from "./src/config.ts";
import { CohostContext, POST_URL_REGEX } from "./src/context.ts";
import { loadAllLikedPosts } from "./src/likes.ts";
import { FROM_POST_PAGE_TO_ROOT, loadPostPage } from "./src/post-page.ts";
import { loadAllProjectPosts } from "./src/project.ts";
import { IPost } from "./src/model.ts";
import { readDataPortabilityArchiveItems } from "./src/data-portability-archive.ts";
import { loadCohostSource } from "./src/cohost-source.ts";
import { generateAllScripts } from "./src/scripts/index.ts";
import { rewritePost } from "./src/post.ts";
import { generateAllIndices } from "./src/post-index.ts";
import { checkForUpdates } from "./src/changelog.ts";

await checkForUpdates();

const ctx = new CohostContext(COOKIE, "out");
await ctx.init();

let isLoggedIn = false;
{
    // check that login actually worked
    const loginStateResponse = await ctx.get(
        "https://cohost.org/api/v1/trpc/login.loggedIn?batch=1&input=%7B%7D",
    );
    const loginState = await loginStateResponse.json();
    if (!loginState[0].result.data.loggedIn) {
        console.error(
            "\x1b[33mwarning:\nNot logged in. Please update your cookie configuration if cohost.org still exists\x1b[m\n\n",
        );
    } else {
        console.log(`logged in as ${loginState[0].result.data.email}`);
        isLoggedIn = true;
    }
}

// JSON data
if (isLoggedIn) {
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
} else {
    console.log("\x1b[33mnot logged in: skipping liked posts and project posts \x1b[m");
}

// javascript
if (ENABLE_JAVASCRIPT) {
    const dir = await loadCohostSource(ctx);
    await generateAllScripts(ctx, dir);
}

const errors: { url: string; error: Error }[] = [];

// Single post pages
{
    const likedPosts: IPost[] = [];
    if (await ctx.hasFile("liked.json")) {
        likedPosts.push(...await ctx.readJson("liked.json"));
    }
    const projectPosts = await Promise.all(
        PROJECTS.map(async (handle) => {
            const file = `${handle}/posts.json`;
            if (await ctx.hasFile(file)) {
                return ctx.readJson(`${handle}/posts.json`)
            } else {
                return [];
            }
        }),
    ) as IPost[][];

    const allPosts = [
        ...likedPosts,
        ...projectPosts.flatMap((x) => x),
    ];

    const loadPostPageAndCollectError = async (url: string) => {
        try {
            await loadPostPage(ctx, url);
        } catch (error) {
            console.error(`\x1b[31mFailed! ${error}\x1b[m`);
            errors.push({ url, error });
        }
    };

    for (const post of allPosts) {
        if (SKIP_POSTS.includes(post.postId)) continue;

        console.log(`~~ processing post ${post.singlePostPageUrl}`);
        await loadPostPageAndCollectError(post.singlePostPageUrl);
    }

    // it can happen that we've cached data for a post that is now a 404.
    // I suppose we can try loading resources for those as well?
    for (const post of allPosts) {
        try {
            await rewritePost(ctx, post, FROM_POST_PAGE_TO_ROOT);
        } catch {
            // oh well!!
        }
    }

    const dpaPostURLs: string[] = [];
    if (DATA_PORTABILITY_ARCHIVE_PATH) {
        const items = await readDataPortabilityArchiveItems(
            DATA_PORTABILITY_ARCHIVE_PATH,
        );
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
        const probablyThePostId = +(post.match(POST_URL_REGEX)?.[2] || "");
        if (SKIP_POSTS.includes(probablyThePostId)) continue;

        console.log(`~~ processing additional post ${post}`);
        await loadPostPageAndCollectError(post);
    }
}

{
    await generateAllIndices(ctx);
}

await ctx.finalize();

if (errors.length) {
    console.log(
        `\x1b[32mDone, \x1b[33mwith ${errors.length} error${errors.length === 1 ? "" : "s"}\x1b[m`,
    );
    for (const { url, error } of errors) console.log(`${url}: ${error}`);
} else {
    console.log("\x1b[32mDone\x1b[m");
}
