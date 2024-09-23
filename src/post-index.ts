import * as path from "jsr:@std/path";
import { CohostContext } from "./context.ts";
import { Document, DOMParser } from "jsr:@b-fuze/deno-dom";
import MiniSearch from "npm:minisearch@7.1.0";
import {
    GENERIC_DISPLAY_PREFS,
    getPageState,
    IDisplayPrefs,
    IPost,
    IProject,
} from "./model.ts";
import { ISinglePostView } from "./post-page.ts";
import { PROJECT_INDEX_SCRIPT_PATH } from "./scripts/index.ts";
import { rewritePost } from "./post.ts";
import { GENERIC_OBSERVER } from "./config.ts";
import { rewriteProject } from "./project.ts";
import {
    IPostSearchData,
    PAGE_STRIDE,
    PostSearchFlags,
} from "./scripts/shared.ts";

const PROJECT_TO_ROOT = "../";

const allPostsHtmlTemplate = (headElements: string, data: string) =>
    `<!doctype html>
<html lang="en">
    <head>
        <meta charset="UTF-8" />
        <title>cohost archive</title>
        ${headElements}
    </head>
    <body class="text-text bg-background overflow-x-hidden lg:overflow-x-auto mt-0">
        <noscript>Javascript is required</noscript>
        <div id="app"></div>
        <script type="application/json" id="post-index-data">${data}</script>
        <script src="${PROJECT_TO_ROOT + PROJECT_INDEX_SCRIPT_PATH}"></script>
    </body>
</html>
`;

const projectHtmlTemplate = (
    headElements: string,
    project: IProject,
    data: string,
) => `<!doctype html>
<html lang="en">
    <head>
        <meta charset="UTF-8" />
        <title>@${project.handle} - cohost archive</title>
        ${headElements}
    </head>
    <body class="text-text bg-background overflow-x-hidden lg:overflow-x-auto mt-0">
        <noscript>Javascript is required</noscript>
        <div id="app"></div>
        <script type="application/json" id="post-index-data">${data}</script>
        <script src="${PROJECT_TO_ROOT + PROJECT_INDEX_SCRIPT_PATH}"></script>
    </body>
</html>
`;

const indexJsTemplate = (data: string) =>
    `window.cohostDL.postSearchIndex(\`${
        data
            .replace(/\\/g, "\\\\")
            .replace(/`/g, "\\`")
            .replace(/\$\{/g, "\\${")
    }\`);`;

const chunkJsTemplate = (id: string, posts: string, rewriteData: string) =>
    `window.cohostDL.postListChunk(
    ${JSON.stringify(id)},
    ${posts},
    ${rewriteData},
);`;

async function getHeadElements(ctx: CohostContext, document: Document) {
    return (await Promise.all([
        ...document.querySelectorAll('html head link[rel="stylesheet"]'),
        ...document.querySelectorAll("html head style"),
        ...document.querySelectorAll('html head meta[name="theme-color"]'),
        ...document.querySelectorAll('html head meta[name="robots"]'),
        ...document.querySelectorAll('html head meta[name="viewport"]'),
        ...document.querySelectorAll('html head script[id="initialI18nStore"]'),
        ...document.querySelectorAll('html head script[id="initialLanguage"]'),
    ].map(async (item) => {
        if (
            item.tagName === "LINK" && item.getAttribute("rel") === "stylesheet"
        ) {
            const resolved = new URL(
                item.getAttribute("href")!,
                "https://cohost.org/handle/post/x",
            );
            item.setAttribute(
                "href",
                PROJECT_TO_ROOT +
                    await ctx.loadResourceToFile(resolved.toString()),
            );
        }
        return item;
    }))).map((item) => item.outerHTML).join("\n");
}

const PROJECT_REQUIRED_TRPC_QUERIES = [
    "users.displayPrefs",
    "projects.isReaderMuting",
    "projects.isReaderBlocking",
];

async function getProjectPageData(ctx: CohostContext, document: Document) {
    const headElements = await getHeadElements(ctx, document);

    const pageState = getPageState<ISinglePostView>(
        document,
        "single-post-view",
    );
    const project = pageState.state.project;

    if (GENERIC_OBSERVER) {
        const displayPrefs = pageState.query<IDisplayPrefs>(
            "users.displayPrefs",
        );
        Object.assign(displayPrefs, GENERIC_DISPLAY_PREFS);
    }

    return {
        headElements,
        project,
        trpcState: {
            ...pageState.trpcState,
            queries: pageState.trpcState.queries.filter((query) => {
                const key = typeof query.queryKey[0] === "string"
                    ? query.queryKey[0]
                    : query.queryKey[0].join?.(".");
                return PROJECT_REQUIRED_TRPC_QUERIES.includes(key);
            }),
        },
    };
}

function toBatches<T>(items: T[], size: number): T[][] {
    const batches: T[][] = [[]];
    for (const item of items) {
        if (batches.at(-1)!.length >= size) batches.push([]);
        batches.at(-1)!.push(item);
    }
    return batches;
}

function flagsForPost(post: IPost): PostSearchFlags {
    let flags = 0;
    if (post.responseToAskId) flags |= PostSearchFlags.AskResponse;
    if (post.effectiveAdultContent) flags |= PostSearchFlags.AdultContent;

    if (post.transparentShareOfPostId) flags |= PostSearchFlags.Share;
    else if (post.shareOfPostId) flags |= PostSearchFlags.Reply;

    if (post.pinned) flags |= PostSearchFlags.Pinned;

    if (post.isEditor) flags |= PostSearchFlags.Editor;
    if (post.isLiked) flags |= PostSearchFlags.Liked;

    return flags;
}

interface Batch {
    index: number;
    posts: IPost[];
    indexablePosts: IPostSearchData[];
    searchTreeIndex: Record<string, number[]>;
    rewriteData: Record<string, string>;
}

type IndexBatch = Pick<Batch, "indexablePosts" | "searchTreeIndex">;

async function readPostFileBatch(
    ctx: CohostContext,
    projectPostsDir: string,
    projectHandle: string,
    batch: string[],
    index: number,
): Promise<Batch> {
    const posts = await Promise.all(batch.map(async (item) => {
        const html = await Deno.readTextFile(path.join(projectPostsDir, item));
        const document = new DOMParser().parseFromString(html, "text/html");

        const pageState = getPageState<ISinglePostView>(
            document,
            "single-post-view",
        );

        const { post } = pageState.query<{ post: IPost }>("posts.singlePost", {
            handle: pageState.state.project.handle,
            postId: pageState.state.postId,
        });

        return post;
    }));

    const rewriteData = await Promise.all(
        posts.map((post) => rewritePost(ctx, post, PROJECT_TO_ROOT)),
    );

    for (const post of posts) {
        post.singlePostPageUrl =
            `../${projectHandle}/post/${post.filename}.html`;
    }

    const indexablePosts: IPostSearchData[] = [];
    const searchTreeIndex: Record<number, number[]> = {};

    const addPost = (post: IPost, tree: number) => {
        indexablePosts.push({
            id: post.postId,
            author: post.postingProject.handle,
            contents: [post.headline, post.plainTextBody].join("\n"),
            tags: post.tags.join("\n"),
            published: post.publishedAt,
            flags: flagsForPost(post),
            chunk: `${projectHandle}~${index}`,
            isRoot: tree === post.postId,
        });

        if (searchTreeIndex[post.postId]) {
            searchTreeIndex[post.postId].push(tree);
        } else searchTreeIndex[post.postId] = [tree];
    };

    for (const post of posts) {
        addPost(post, post.postId);

        for (const p of post.shareTree) {
            addPost(p, post.postId);
        }
    }

    return {
        index,
        posts,
        indexablePosts,
        searchTreeIndex,
        rewriteData: Object.assign({}, ...rewriteData.map((item) => item.urls)),
    };
}

function sortPostFileNames(postFileNames: string[]) {
    // newest to oldest
    postFileNames.sort((a, b) => {
        const aMatch = a.match(/^(\d+)-/);
        const bMatch = b.match(/^(\d+)-/);
        if (aMatch && bMatch) {
            return +bMatch[1] - +aMatch[1];
        }
        return b.localeCompare(a);
    });
}

function createSearch() {
    return new MiniSearch<IPostSearchData>({
        idField: "id",
        fields: ["author", "contents", "tags"],
        storeFields: ["author", "published", "tags", "flags", "chunk"],
        processTerm: (term: string, fieldName?: string) => {
            if (fieldName === "contents") {
                // cut these off. we don't need to index endless base64 strings
                return [...term.toLowerCase().substring(0, 100)].slice(0, 50)
                    .join("");
            }

            return term.toLowerCase();
        },
        tokenize: (text: string, fieldName?: string) => {
            if (fieldName === "author") return [text];
            if (fieldName === "tags") return text.split(/\n/);

            // MiniSearch default tokenizer
            return text.split(/[\n\r\p{Z}\p{P}]+/u);
        },
    });
}

export async function generateProjectIndex(
    ctx: CohostContext,
    projectHandle: string,
) {
    console.log(`generating index for ${projectHandle}`);

    {
        const projectDir = ctx.getCleanPath(projectHandle);

        const clearProjectDirItems: string[] = [];
        for await (const item of Deno.readDir(projectDir)) {
            if (
                item.name === "cdl-index.js" ||
                item.name.startsWith("cdl-chunk") && item.name.endsWith(".js")
            ) {
                clearProjectDirItems.push(item.name);
            }
        }
        for (const item of clearProjectDirItems) {
            await Deno.remove(path.join(projectDir, item));
        }
    }

    const projectPostsDir = ctx.getCleanPath(path.join(projectHandle, "post"));

    const postFileNames: string[] = [];
    for await (const item of Deno.readDir(projectPostsDir)) {
        const match = item.name.match(/^(\d+-[-\w]+)[.]html$/);
        if (item.isFile && match) {
            postFileNames.push(match[1]);
        }
    }

    if (!postFileNames.length) {
        console.log("never mind - there’s no data");
        return [];
    }

    sortPostFileNames(postFileNames);

    const postFileBatches: string[][] = toBatches(postFileNames, PAGE_STRIDE);

    // get project data from the newest post
    const { headElements, project, trpcState } = await (async () => {
        const html = await Deno.readTextFile(
            path.join(projectPostsDir, postFileNames[0]),
        );
        const document = new DOMParser().parseFromString(html, "text/html");
        return getProjectPageData(ctx, document);
    })();

    const postBatches: Batch[] = [];
    for (const batch of postFileBatches) {
        postBatches.push(
            await readPostFileBatch(
                ctx,
                projectPostsDir,
                project.handle,
                batch,
                postBatches.length,
            ),
        );
    }

    const projectRewriteData = await rewriteProject(
        ctx,
        project,
        PROJECT_TO_ROOT,
    );

    const rewriteData = {
        base: `https://cohost.org/${project.handle}/post/a`,
        urls: projectRewriteData,
    };

    const search = createSearch();
    const searchTreeIndex: Record<string, number[]> = {};
    const seenPosts = new Set<number>();

    // add tree roots first
    for (const batch of postBatches) {
        for (const post of batch.indexablePosts) {
            if (post.isRoot && !seenPosts.has(post.id)) {
                search.add(post);
                seenPosts.add(post.id);
            }
        }
    }

    // add rest & tree index
    for (const batch of postBatches) {
        for (const post of batch.indexablePosts) {
            if (!seenPosts.has(post.id)) {
                search.add(post);
                seenPosts.add(post.id);
            }
        }

        for (const [post, trees] of Object.entries(batch.searchTreeIndex)) {
            if (searchTreeIndex[post]) {
                for (const t of trees) {
                    if (!searchTreeIndex[post].includes(t)) {
                        searchTreeIndex[post].push(t);
                    }
                }
            } else {
                searchTreeIndex[post] = trees;
            }
        }
    }

    const data = {
        project,
        rewriteData,
        searchTreeIndex,
        trpcState: {
            ...trpcState,
            queries: trpcState.queries.filter((query) => {
                const key = typeof query.queryKey[0] === "string"
                    ? query.queryKey[0]
                    : query.queryKey[0].join?.(".");
                return PROJECT_REQUIRED_TRPC_QUERIES.includes(key);
            }),
        },
    };

    for (let i = 0; i < postBatches.length; i += 1) {
        const filePath = ctx.getCleanPath(
            path.join(projectHandle, `cdl-chunk~${project.handle}~${i}.js`),
        );
        await Deno.writeTextFile(
            filePath,
            chunkJsTemplate(
                `${project.handle}~${i}`,
                JSON.stringify(postBatches[i].posts),
                JSON.stringify(postBatches[i].rewriteData),
            ),
        );
    }

    const indexFilePath = ctx.getCleanPath(
        path.join(projectHandle, "cdl-index.js"),
    );
    await Deno.writeTextFile(
        indexFilePath,
        indexJsTemplate(JSON.stringify(search.toJSON())),
    );

    const filePath = ctx.getCleanPath(path.join(projectHandle, "index.html"));
    await Deno.writeTextFile(
        filePath,
        projectHtmlTemplate(
            headElements,
            project,
            JSON.stringify(data).replace(/<\/script>/g, "<\\/script>"),
        ),
    );

    return postBatches.map((batch) => ({
        indexablePosts: batch.indexablePosts,
        searchTreeIndex: batch.searchTreeIndex,
    })) as IndexBatch[];
}

const NOT_PROJECT_NAMES = ["rc", "api", "static"];

export async function generateAllPostsIndex(
    ctx: CohostContext,
    batches: IndexBatch[],
) {
    if (!batches.length) return;

    console.log(`generating index for all posts`);
    const allDir = ctx.getCleanPath("~all");

    try {
        await Deno.remove(allDir, { recursive: true });
    } catch {
        // probably not important
    }

    await Deno.mkdir(allDir);

    const newestPost = batches.flatMap((batch) => batch.indexablePosts)
        .reduce((a, b) => {
            const aPub = new Date(a.published);
            const bPub = new Date(b.published);
            if (bPub > aPub) return b;
            return a;
        });

    const newestPostFile = await ctx.getCachedFileForPost(
        newestPost.author,
        newestPost.id,
    );
    if (!newestPostFile) {
        throw new Error(`bad state: post was in batch but isn’t cached`);
    }

    // get data from the newest post
    const { headElements, trpcState } = await (async () => {
        const html = await Deno.readTextFile(ctx.getCleanPath(newestPostFile));
        const document = new DOMParser().parseFromString(html, "text/html");

        const headElements = await getHeadElements(ctx, document);

        const pageState = getPageState<ISinglePostView>(
            document,
            "single-post-view",
        );

        if (GENERIC_OBSERVER) {
            const displayPrefs = pageState.query<IDisplayPrefs>(
                "users.displayPrefs",
            );
            Object.assign(displayPrefs, GENERIC_DISPLAY_PREFS);
        }

        return {
            headElements,
            trpcState: {
                ...pageState.trpcState,
                queries: pageState.trpcState.queries.filter((query) => {
                    const key = typeof query.queryKey[0] === "string"
                        ? query.queryKey[0]
                        : query.queryKey[0].join?.(".");
                    return ["users.displayPrefs"].includes(key);
                }),
            },
        };
    })();

    const search = createSearch();
    const searchTreeIndex: Record<string, number[]> = {};
    const seenPosts = new Set<number>();
    const chunks: Record<string, string> = {};

    const toBeAdded: IPostSearchData[] = [];

    // add tree roots first
    for (const batch of batches) {
        for (const post of batch.indexablePosts) {
            if (!chunks[post.id]) {
                chunks[post.id] = post.chunk;
            }

            if (post.isRoot && !seenPosts.has(post.id)) {
                toBeAdded.push(post);
                seenPosts.add(post.id);
            }
        }
    }

    // add rest & tree index
    for (const batch of batches) {
        for (const post of batch.indexablePosts) {
            if (!chunks[post.id]) {
                chunks[post.id] = post.chunk;
            }

            if (!seenPosts.has(post.id)) {
                toBeAdded.push(post);
                seenPosts.add(post.id);
            }
        }

        for (const [post, trees] of Object.entries(batch.searchTreeIndex)) {
            if (searchTreeIndex[post]) {
                for (const t of trees) {
                    if (!searchTreeIndex[post].includes(t)) {
                        searchTreeIndex[post].push(t);
                    }
                }
            } else {
                searchTreeIndex[post] = trees;
            }
        }
    }

    // sort by date by default
    toBeAdded.sort((a, b) => {
        const aPub = new Date(a.published);
        const bPub = new Date(b.published);
        return bPub > aPub ? 1 : -1;
    });

    search.addAll(toBeAdded);

    console.log(`that’s ${search.documentCount} posts`);

    const data = {
        chunks,
        searchTreeIndex,
        trpcState,
    };

    await Deno.writeTextFile(
        path.join(allDir, "cdl-index.js"),
        indexJsTemplate(JSON.stringify(search.toJSON())),
    );

    await Deno.writeTextFile(
        path.join(allDir, "index.html"),
        allPostsHtmlTemplate(
            headElements,
            JSON.stringify(data).replace(/<\/script>/g, "<\\/script>"),
        ),
    );

    const allProjects = [
        ...new Set(
            batches
                .flatMap((batch) => batch.indexablePosts)
                .filter((post) => post.isRoot)
                .map((post) => post.author),
        ),
    ].sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));

    // rudimentary root index
    await Deno.writeTextFile(
        ctx.getCleanPath("index.html"),
        `<!doctype html>
<html lang="en">
    <head>
        <meta charset="UTF-8" />
        <title>cohost archive</title>
        <style>
body {
    font-family: system-ui, sans-serif;
    max-width: 50ch;
    margin: 1em auto;
}
a {
    color: rgb(131 37 79);
}
@media (prefers-color-scheme: dark) {
    body {
        background: #191919;
        color: #eee;
    }
    a {
        color: rgb(229 143 62);
    }
}
        </style>
    </head>
    <body>
        <h1>cohost archive</h1>
        <p>
            <a href="~all/index.html">The Cohost Archive Global Feed</a>
        </p>
        <ul>
        ${
            allProjects.map((project) => (
                `<li><a href="${project}/index.html">@${project}</a></li>`
            )).join("\n")
        }
        </ul>
    </body>
</html>
`,
    );
}

export async function generateAllIndices(ctx: CohostContext, errors: { url: string; error: Error }[]) {
    const handles: string[] = [];

    for await (const item of Deno.readDir(ctx.getCleanPath(""))) {
        if (
            item.isDirectory && !item.name.startsWith("~") &&
            !NOT_PROJECT_NAMES.includes(item.name)
        ) {
            handles.push(item.name);
        }
    }

    handles.sort(); // might as well

    const batches: IndexBatch[] = [];
    for (const handle of handles) {
        try {
            batches.push(...await generateProjectIndex(ctx, handle));
        } catch (error) {
            console.error(`\x1b[31mFailed! ${error}\x1b[m`);
            errors.push({ url: `index for ${handle}`, error });
        }
    }

    await generateAllPostsIndex(ctx, batches);
}
