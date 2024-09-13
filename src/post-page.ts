import { Document, Element } from "jsr:@b-fuze/deno-dom";
import {
    generate as cssGenerate,
    parse as cssParse,
    walk as cssWalk,
} from "npm:css-tree@2.3.1";
import { CohostContext, encodeFilePathURI } from "./context.ts";
import {
    COHOST_DL_USER,
    getPageState,
    IComment,
    ILoggedIn,
    IPost,
    IProject,
    savePageState,
} from "./model.ts";
import { POST_PAGE_SCRIPT_PATH } from "./post-page-script.ts";
import { rewritePost } from "./post.ts";
import { rewriteProject } from "./project.ts";
import { rewriteComment } from "./comment.ts";
import { ENABLE_JAVASCRIPT } from "./config.ts";

interface ISinglePostView {
    postId: number;
    project: IProject;
}

export const FROM_POST_PAGE_TO_ROOT = "../../";

async function loadResources(
    ctx: CohostContext,
    document: Document,
    urlBase: string,
    filePathBase: string,
): Promise<Record<string, string>> {
    const rewrites: Record<string, string> = {};

    const loadStylesheets = [...document.querySelectorAll("link")].map(
        async (link) => {
            const href = link.getAttribute("href");

            const resolvedHref = href ? new URL(href, urlBase) : null;

            if (resolvedHref?.protocol === "https:") {
                const filePath = await ctx.loadResourceToFile(
                    resolvedHref.toString(),
                );
                if (filePath) {
                    const url = filePathBase + filePath;
                    link.setAttribute("href", url);
                    rewrites[href!] = url;
                }
            }
        },
    );

    const loadSrcElements = [...document.querySelectorAll("script, img, audio")]
        .map(
            async (el) => {
                const src = el.getAttribute("src");

                const resolvedSrc = src ? new URL(src, urlBase) : null;

                if (resolvedSrc?.protocol === "https:") {
                    const filePath = await ctx.loadResourceToFile(
                        resolvedSrc.toString(),
                    );
                    if (filePath) {
                        const url = encodeFilePathURI(filePathBase + filePath);
                        el.setAttribute("src", url);
                        rewrites[src!] = url;
                        el.removeAttribute("srcset");

                        if (el.tagName === "AUDIO") {
                            // add controls, because JS playback doesn't work at the moment
                            el.setAttribute("controls", "");
                        } else if (el.tagName === "SCRIPT") {
                            // remove scripts
                            el.setAttribute(
                                "data-original-src",
                                el.getAttribute("src"),
                            );
                            el.removeAttribute("src");
                        }
                    }
                }
            },
        );

    await Promise.all([...loadStylesheets, ...loadSrcElements]);

    const visit = async (el: Element) => {
        const styleAttr = el.getAttribute("style");
        if (styleAttr) {
            const tree = cssParse(styleAttr, {
                context: "declarationList",
            });

            const nodes: { value: string }[] = [];
            cssWalk(tree, (node: { type: string; value: string }) => {
                if (node.type === "Url") {
                    nodes.push(node);
                }
            });
            await Promise.all(nodes.map(async (node) => {
                const resolved = new URL(node.value, urlBase);
                if (resolved.protocol !== "https:") return;

                const filePath = await ctx.loadResourceToFile(
                    resolved.toString(),
                );
                if (filePath) node.value = encodeFilePathURI(filePathBase + filePath);
            }));

            el.setAttribute("style", cssGenerate(tree));
        }

        await Promise.all([...el.children].map(visit));
    };

    await Promise.all(
        [...document.querySelectorAll("[data-post-body][class]")].map(visit),
    );

    return rewrites;
}

export function filePathForPost(post: IPost): string {
    return `${post.postingProject.handle}/post/${post.filename}.html`;
}

export async function loadPostPage(ctx: CohostContext, url: string) {
    // it's kind of an accident that we also store the original file (it's a <link rel="canonical">),
    // but we might as well repurpose it here as a cache mechanism
    const cachedFilePath = await ctx.getCachedFileForPostURL(url);
    const cachedOriginalPath = cachedFilePath?.replace(/[.]html$/, "");

    const document = await ctx.getDocument(url, cachedOriginalPath);

    const pageRewrites = await loadResources(
        ctx,
        document,
        url,
        FROM_POST_PAGE_TO_ROOT,
    );

    const contentScript = document.createElement("script");
    contentScript.setAttribute(
        "src",
        FROM_POST_PAGE_TO_ROOT + POST_PAGE_SCRIPT_PATH,
    );
    contentScript.setAttribute("async", "");
    if (ENABLE_JAVASCRIPT) {
        document.body.append(contentScript);
    }

    for (const link of document.querySelectorAll("link")) {
        if (
            link.getAttribute("rel") === "preload" &&
            link.getAttribute("href")?.endsWith(".js")
        ) {
            link.remove();
        }
    }

    const pageState = getPageState<ISinglePostView>(
        document,
        "single-post-view",
    );

    const { post, comments } = pageState.query<
        { post: IPost; comments: Record<string, IComment[]> }
    >(
        "posts.singlePost",
        {
            handle: pageState.state.project.handle,
            postId: pageState.state.postId,
        },
    );

    // remove login info
    Object.assign(pageState.query<ILoggedIn>("login.loggedIn"), COHOST_DL_USER);

    const rewriteData = await rewritePost(ctx, post, FROM_POST_PAGE_TO_ROOT);

    const editedProjects = pageState.query<{ projects: IProject[] }>(
        "projects.listEditedProjects",
    );
    for (const project of editedProjects.projects) {
        Object.assign(
            rewriteData.urls,
            await rewriteProject(ctx, project, FROM_POST_PAGE_TO_ROOT),
        );
    }

    Object.assign(
        rewriteData.urls,
        await rewriteProject(
            ctx,
            pageState.state.project,
            FROM_POST_PAGE_TO_ROOT,
        ),
    );

    for (const comment of Object.values(comments).flatMap((x) => x)) {
        Object.assign(
            rewriteData.urls,
            await rewriteComment(ctx, comment, FROM_POST_PAGE_TO_ROOT),
        );
    }

    Object.assign(rewriteData.urls, pageRewrites);

    const rewriteScript = document.createElement("script");
    rewriteScript.setAttribute("type", "application/json");
    rewriteScript.setAttribute("id", "__cohost_dl_rewrite_data");
    rewriteScript.innerHTML = JSON.stringify(rewriteData);
    document.head.append(rewriteScript);

    savePageState(document, pageState);

    fixReactHydration(document);

    await ctx.write(
        filePathForPost(post),
        "<!DOCTYPE html>\n" + document.documentElement!.outerHTML,
    );
}

function fixReactHydration(document: Document) {
    const singlePostViewParent = document.querySelector("div.flex.flex-grow.flex-col.pb-20")!;
    const divForSomeReason = document.createElement('div');
    singlePostViewParent.insertBefore(divForSomeReason, singlePostViewParent.childNodes[1]);

    // TODO: consider continuing?
    // current state: it at least doesn't delete the entire DOM during hydration.
    // As it turns out, full hydration is broken on actual real cohost.org as well, so maybe this is not really necessary...
}
