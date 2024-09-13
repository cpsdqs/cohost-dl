import { CohostContext, encodeFilePathURI } from "./context.ts";
import { unified } from "npm:unified";
import remarkParse from "npm:remark-parse";
import remarkGfm from "npm:remark-gfm";
import remarkStringify from "npm:remark-stringify";

interface MdastNode {
    type: "image" | string;
    children?: MdastNode[];
    url?: string;
}

async function rewriteUrls(
    ctx: CohostContext,
    node: MdastNode,
    base: string,
    rewrites: Record<string, string>,
) {
    if (node.type === "image" && node.url) {
        const resolved = new URL(node.url, 'https://cohost.org/x').toString();
        const url = await ctx.loadResourceToFile(resolved);
        if (url) {
            rewrites[node.url] = encodeFilePathURI(base + url);
            node.url = encodeFilePathURI(base + url);
        }
    }

    for (const child of node.children ?? []) {
        await rewriteUrls(ctx, child, base, rewrites);
    }
}

export async function rewriteMarkdownString(
    ctx: CohostContext,
    markdown: string,
    base: string,
): Promise<{ markdown: string; urls: Record<string, string> }> {
    const rewrites: Record<string, string> = {};

    const ast = unified()
        .use(remarkParse)
        .use(remarkGfm, { singleTilde: false })
        .parse(markdown);

    await rewriteUrls(ctx, ast as MdastNode, base, rewrites);

    const result = unified().use(remarkGfm, { singleTilde: false }).use(
        remarkStringify,
    ).stringify(ast);

    return { markdown: result, urls: rewrites };
}
