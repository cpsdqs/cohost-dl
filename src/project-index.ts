import * as path from "jsr:@std/path";
import { CohostContext } from "./context.ts";
import { DOMParser } from "jsr:@b-fuze/deno-dom";
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

const TO_ROOT = "../";

const htmlTemplate = (
    headElements: string,
    project: IProject,
    data: string,
    rewriteData: string,
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
        <script type="application/json" id="__cohost_dl_rewrite_data">${rewriteData}</script>
        <script src="${TO_ROOT + PROJECT_INDEX_SCRIPT_PATH}"></script>
    </body>
</html>
`;

const REQUIRED_TRPC_QUERIES = [
    "users.displayPrefs",
    "projects.isReaderMuting",
    "projects.isReaderBlocking",
];

export async function generateProjectIndex(
    ctx: CohostContext,
    projectHandle: string,
) {
    console.log(`generating index for ${projectHandle}`);

    const projectPostsDir = ctx.getCleanPath(path.join(projectHandle, "post"));

    const postFileNames: string[] = [];
    for await (const item of Deno.readDir(projectPostsDir)) {
        const match = item.name.match(/^(\d+-[-\w]+)[.]html$/);
        if (item.isFile && match) {
            postFileNames.push(match[1]);
        }
    }

    const posts = await Promise.all(postFileNames.map(async (item) => {
        const html = await Deno.readTextFile(path.join(projectPostsDir, item));
        const document = new DOMParser().parseFromString(html, "text/html");

        const pageState = getPageState<ISinglePostView>(
            document,
            "single-post-view",
        );

        const project = pageState.state.project;
        const { post } = pageState.query<{ post: IPost }>("posts.singlePost", {
            handle: project.handle,
            postId: pageState.state.postId,
        });

        if (GENERIC_OBSERVER) {
            const displayPrefs = pageState.query<IDisplayPrefs>(
                "users.displayPrefs",
            );
            Object.assign(displayPrefs, GENERIC_DISPLAY_PREFS);
        }

        return {
            project,
            post,
            document,
            trpcState: pageState.trpcState,
        };
    }));

    const project = posts[0]?.project;
    const document = posts[0]?.document;
    const trpcState = posts[0]?.trpcState;
    if (!project || !document || !trpcState) {
        console.log("never mind - there’s no data");
        return;
    }

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

    const postRewriteData = await Promise.all(
        posts.map((item) => rewritePost(ctx, item.post, TO_ROOT)),
    );

    for (const { post } of posts) {
        post.singlePostPageUrl = `./post/${post.filename}.html`;
    }

    const projectRewriteData = await rewriteProject(ctx, project, TO_ROOT);

    const data = {
        project,
        posts: posts.map((item) => item.post),
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

    const rewriteData = {
        base: postRewriteData[0].base,
        urls: Object.assign(
            {},
            projectRewriteData,
            ...postRewriteData.map((item) => item.urls),
        ),
    };

    const filePath = ctx.getCleanPath(path.join(projectHandle, "index.html"));
    await Deno.writeTextFile(
        filePath,
        htmlTemplate(
            headElements,
            project,
            JSON.stringify(data),
            JSON.stringify(rewriteData),
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
