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
import { IPostSearchData, PAGE_STRIDE, PostSearchFlags } from "./scripts/shared.ts";

const TO_ROOT = "../";

const htmlTemplate = (
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
        <script type="application/json" id="project-index-data">${data}</script>
        <script src="${TO_ROOT + PROJECT_INDEX_SCRIPT_PATH}"></script>
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

const REQUIRED_TRPC_QUERIES = [
    "users.displayPrefs",
    "projects.isReaderMuting",
    "projects.isReaderBlocking",
];

async function getProjectPageData(ctx: CohostContext, document: Document) {
    const headElements = (await Promise.all([
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
                TO_ROOT + await ctx.loadResourceToFile(resolved.toString()),
            );
        }
        return item;
    }))).map((item) => item.outerHTML).join("\n");

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
                return REQUIRED_TRPC_QUERIES.includes(key);
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
            handle: projectHandle,
            postId: pageState.state.postId,
        });

        return post;
    }));

    const rewriteData = await Promise.all(
        posts.map((post) => rewritePost(ctx, post, TO_ROOT)),
    );

    for (const post of posts) {
        post.singlePostPageUrl = `./post/${post.filename}.html`;
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
        return;
    }

    // newest to oldest
    postFileNames.sort((a, b) => {
        const aMatch = a.match(/^(\d+)-/);
        const bMatch = b.match(/^(\d+)-/);
        if (aMatch && bMatch) {
            return +bMatch[1] - +aMatch[1];
        }
        return b.localeCompare(a);
    });

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

    const projectRewriteData = await rewriteProject(ctx, project, TO_ROOT);

    const rewriteData = {
        base: `https://cohost.org/${project.handle}/post/a`,
        urls: projectRewriteData,
    };

    const search = new MiniSearch<IPostSearchData>({
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
    const searchTreeIndex: Record<string, number[]> = {};
    const seenPosts = new Set<number>();

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
                return REQUIRED_TRPC_QUERIES.includes(key);
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
        htmlTemplate(
            headElements,
            project,
            JSON.stringify(data).replace(/<\/script>/g, "<\\/script>"),
        ),
    );
}

const NOT_PROJECT_NAMES = ["rc", "api", "static"];

export async function generateAllProjectIndices(ctx: CohostContext) {
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

    for (const handle of handles) {
        await generateProjectIndex(ctx, handle);
    }
}
