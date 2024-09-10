import { Document, Element } from "jsr:@b-fuze/deno-dom";
import {
    generate as cssGenerate,
    parse as cssParse,
    walk as cssWalk,
} from "npm:css-tree@2.3.1";
import { CohostContext } from "./context.ts";
import { getPageState, IPost, IProject } from "./model.ts";

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

    const loadImages = [...document.querySelectorAll("img")].map(
        async (img) => {
            const src = img.getAttribute("src");

            const resolvedSrc = src ? new URL(src, urlBase) : null;

            if (resolvedSrc?.protocol === "https:") {
                const filePath = await ctx.loadResourceToFile(
                    resolvedSrc.toString(),
                );
                if (filePath) {
                    img.setAttribute("src", encodeURI(filePathBase + filePath));
                    img.removeAttribute("srcset");
                }
            }
        },
    );
    await Promise.all([...loadStylesheets, ...loadImages]);

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
    const document = await ctx.getDocument(url);

    await loadResources(ctx, document, url, FROM_POST_PAGE_TO_ROOT);

    const pageState = getPageState<ISinglePostView>(
        document,
        "single-post-view",
    );

    const { post } = pageState.query<{ post: IPost }>("posts.singlePost", {
        handle: pageState.state.project.handle,
        postId: pageState.state.postId,
    });

    await ctx.write(
        filePathForPost(post),
        "<!DOCTYPE html>\n" + document.documentElement!.outerHTML,
    );
}
