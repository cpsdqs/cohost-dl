import { CohostContext, encodeFilePathURI } from "./context.ts";
import { IPost } from "./model.ts";
import {
    generate as cssGenerate,
    parse as cssParse,
    walk as cssWalk,
} from "npm:css-tree@2.3.1";
import { rewriteProject } from "./project.ts";

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

export async function rewritePost(
    ctx: CohostContext,
    post: IPost,
    base: string,
): Promise<PostRewriteData> {
    const rewriteData: PostRewriteData = {
        base: post.singlePostPageUrl,
        urls: {},
    };

    Object.assign(
        rewriteData.urls,
        await rewriteProject(ctx, post.postingProject, base),
    );

    for (const project of post.relatedProjects) {
        Object.assign(
            rewriteData.urls,
            await rewriteProject(ctx, project, base),
        );
    }

    await Promise.all(post.blocks.map(async (block) => {
        if (block.type === "attachment") {
            const filePath = await ctx.loadResourceToFile(
                block.attachment.fileURL,
            );
            if (filePath) {
                const url = encodeFilePathURI(base + filePath);
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
                            const url = encodeFilePathURI(base + filePath);
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
                            const url = encodeFilePathURI(base + filePath);
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

    for (const item of post.shareTree) {
        const res = await rewritePost(ctx, item, base);
        Object.assign(rewriteData.urls, res.urls);
    }

    return rewriteData;
}
