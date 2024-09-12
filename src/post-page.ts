import { Document, Element } from "jsr:@b-fuze/deno-dom";
import {
    generate as cssGenerate,
    parse as cssParse,
    walk as cssWalk,
} from "npm:css-tree@2.3.1";
import { CohostContext } from "./context.ts";
import { getPageState, IPost, IProject, savePageState } from "./model.ts";
import { POST_PAGE_SCRIPT_PATH } from "./post-page-script.ts";

interface ISinglePostView {
    postId: number;
    project: IProject;
}

const FROM_POST_PAGE_TO_ROOT = "../../";

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
                        const url = encodeURI(filePathBase + filePath);
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
                if (filePath) node.value = encodeURI(filePathBase + filePath);
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
    document.body.append(contentScript);

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

    const { post } = pageState.query<{ post: IPost }>("posts.singlePost", {
        handle: pageState.state.project.handle,
        postId: pageState.state.postId,
    });

    // remove login info
    pageState.updateQuery("login.loggedIn", null, {
        activated: true,
        deleteAfter: null,
        email: "cohost-dl@localhost",
        emailVerified: true,
        emailVerifyCanceled: false,
        loggedIn: true,
        modMode: false,
        projectId: 1,
        readOnly: true,
        twoFactorActive: true,
        userId: 1,
    }, true);

    const rewriteData = await loadPostResources(ctx, post);

    for (const [k, v] of Object.entries(pageRewrites)) rewriteData.urls[k] = v;

    const rewriteScript = document.createElement("script");
    rewriteScript.setAttribute("type", "application/json");
    rewriteScript.setAttribute("id", "__cohost_dl_rewrite_data");
    rewriteScript.innerHTML = JSON.stringify(rewriteData);
    document.head.append(rewriteScript);

    savePageState(document, pageState);

    await ctx.write(
        filePathForPost(post),
        "<!DOCTYPE html>\n" + document.documentElement!.outerHTML,
    );
}

interface ASTPosition {
    line: number;
    column: number;
    offset: number;
}

interface ASTPositionRange {
    start: ASTPosition;
    end: ASTPosition;
}

interface ASTNodeElement {
    type: "element";
    tagName: string;
    properties: Record<string, string>;
    position: ASTPositionRange;
    children: ASTNode[];
}

interface ASTNodeText {
    type: "text";
    value: string;
    position: ASTPositionRange;
}

interface ASTNodeRoot {
    type: "root";
    children: ASTNode[];
    data: { quirksMode: boolean };
    position: ASTPositionRange;
}

type ASTNode = ASTNodeRoot | ASTNodeElement | ASTNodeText;

interface PostRewriteData {
    base: string;
    urls: Record<string, string>;
}

export async function loadPostResources(
    ctx: CohostContext,
    post: IPost,
): Promise<PostRewriteData> {
    const rewriteData: PostRewriteData = {
        base: post.singlePostPageUrl,
        urls: {},
    };

    await Promise.all(post.blocks.map(async (block) => {
        if (block.type === "attachment") {
            const filePath = await ctx.loadResourceToFile(
                block.attachment.fileURL,
            );
            if (filePath) {
                const url = FROM_POST_PAGE_TO_ROOT + filePath;
                rewriteData.urls[block.attachment.fileURL] = url;
                block.attachment.fileURL = url;
                block.attachment.previewURL = url;
            }
        }
    }));

    for (const span of post.astMap.spans) {
        const ast = JSON.parse(span.ast);

        const process = async (node: ASTNode) => {
            if ("properties" in node) {
                if ("style" in node.properties) {
                    const tree = cssParse(node.properties.style, {
                        context: "declarationList",
                    });

                    const nodes: { value: string }[] = [];
                    cssWalk(tree, (node: { type: string; value: string }) => {
                        if (node.type === "Url") {
                            nodes.push(node);
                        }
                    });

                    let mutated = false;
                    await Promise.all(nodes.map(async (node) => {
                        const resolved = new URL(
                            node.value,
                            post.singlePostPageUrl,
                        );
                        if (resolved.protocol !== "https:") return;

                        const filePath = await ctx.loadResourceToFile(
                            resolved.toString(),
                        );
                        if (filePath) {
                            const url = FROM_POST_PAGE_TO_ROOT + filePath;
                            rewriteData.urls[node.value] = url;
                            node.value = url;
                            mutated = true;
                        }
                    }));

                    if (mutated) {
                        node.properties.style = cssGenerate(tree);
                    }
                }

                if (
                    ["img", "audio", "video"].includes(node.tagName) &&
                    node.properties.src
                ) {
                    const resolved = new URL(
                        node.properties.src,
                        post.singlePostPageUrl,
                    );
                    if (resolved.protocol === "https:") {
                        const filePath = await ctx.loadResourceToFile(
                            resolved.toString(),
                        );
                        if (filePath) {
                            const url = FROM_POST_PAGE_TO_ROOT + filePath;
                            rewriteData.urls[node.properties.src] = url;
                            node.properties.src = url;
                        }
                    }
                }
            }

            if ("children" in node) {
                node.children = await Promise.all(node.children.map(process));
            }

            return node;
        };

        span.ast = JSON.stringify(await process(ast));
    }

    return rewriteData;
}
