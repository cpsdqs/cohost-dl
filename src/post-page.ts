import { Document, Element } from "jsr:@b-fuze/deno-dom";
import {
    generate as cssGenerate,
    parse as cssParse,
    walk as cssWalk,
} from "npm:css-tree@2.3.1";
import { CohostContext } from "./context.ts";
import { getPageState, IPost, IProject } from "./model.ts";
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
) {
    const loadStylesheets = [...document.querySelectorAll("link")].map(
        async (link) => {
            const href = link.getAttribute("href");

            const resolvedHref = href ? new URL(href, urlBase) : null;

            if (resolvedHref?.protocol === "https:") {
                const filePath = await ctx.loadResourceToFile(
                    resolvedHref.toString(),
                );
                if (filePath) {
                    link.setAttribute("href", filePathBase + filePath);
                }
            }
        },
    );

    const loadSrcElements = [...document.querySelectorAll("script, img, audio")].map(
        async (el) => {
            const src = el.getAttribute("src");

            const resolvedSrc = src ? new URL(src, urlBase) : null;

            if (resolvedSrc?.protocol === "https:") {
                const filePath = await ctx.loadResourceToFile(
                    resolvedSrc.toString(),
                );
                if (filePath) {
                    el.setAttribute("src", encodeURI(filePathBase + filePath));
                    el.removeAttribute("srcset");

                    if (el.tagName === "AUDIO") {
                        // add controls, because JS playback doesn't work at the moment
                        el.setAttribute("controls", "");
                    } else if (el.tagName === "SCRIPT") {
                        // remove scripts
                        el.setAttribute("data-original-src", el.getAttribute("src"));
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
}

export function filePathForPost(post: IPost): string {
    return `${post.postingProject.handle}/post/${post.filename}.html`;
}

export async function loadPostPage(ctx: CohostContext, url: string) {
    // it's kind of an accident that we also store the original file (it's a <link rel="canonical">),
    // but we might as well repurpose it here as a cache mechanism
    const cachedFilePath = await ctx.getCachedFileForPostURL(url);
    const cachedOriginalPath = cachedFilePath?.replace(/[.]html$/, '');

    const document = await ctx.getDocument(url, cachedOriginalPath);

    await loadResources(ctx, document, url, FROM_POST_PAGE_TO_ROOT);

    const contentScript = document.createElement("script");
    contentScript.setAttribute("src", FROM_POST_PAGE_TO_ROOT + POST_PAGE_SCRIPT_PATH);
    contentScript.setAttribute("async", "");
    document.body.append(contentScript);

    for (const link of document.querySelectorAll("link")) {
        if (link.getAttribute("rel") === "preload" && link.getAttribute("href")?.endsWith(".js")) {
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

    // if the post is behind a CW, it will probably not render on the post page.
    // we should load resources for those as well
    await loadPostResources(ctx, post);

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

export async function loadPostResources(ctx: CohostContext, post: IPost) {
    await Promise.all(post.blocks.map(async block => {
        if (block.type === "attachment") {
            await ctx.loadResourceToFile(block.attachment.fileURL);
        }
    }));

    for (const span of post.astMap.spans) {
        const ast = JSON.parse(span.ast);

        const visit = async (node: ASTNode) => {
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

                    await Promise.all(nodes.map(async (node) => {
                        const resolved = new URL(node.value, post.singlePostPageUrl);
                        if (resolved.protocol !== "https:") return;
                        await ctx.loadResourceToFile(resolved.toString());
                    }));
                }
            }

            if ("children" in node) {
                await Promise.all(node.children.map(visit));
            }
        }

        await visit(ast);
    }
}
