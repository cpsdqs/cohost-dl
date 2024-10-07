import { renderToString } from "react-dom/server";
import { renderMarkdownReactNoHTML } from "./cohost/lib/markdown/other-rendering";
import { generatePostAst } from "./cohost/lib/markdown/post-rendering";
import { PostBody } from "./cohost/preact/components/posts/post-body";
import { ViewBlock } from "./cohost/shared/types/post-blocks";
import { PostId } from "./cohost/shared/types/ids";

const global = globalThis as Record<string, unknown>;

interface PostRenderRequest {
    blocks: ViewBlock[];
    publishedAt: string;
    hasCohostPlus: boolean;
    disableEmbeds: boolean;
    externalLinksInNewTab: boolean;
    resources: Record<string, string>;
}

interface PostResult {
    preview: string;
    full: string;
}

interface MarkdownRenderRequest {
    markdown: string;
}

interface MarkdownResult {
    html: string;
}

export async function renderPost(args: PostRenderRequest): Promise<PostResult> {
    const postAst = await generatePostAst(args.blocks, new Date(args.publishedAt), {
        hasCohostPlus: args.hasCohostPlus,
        renderingContext: "post",
    });

    // TODO: process post blocks & AST

    const preview = renderToString(
        <PostBody
            viewModel={{
                postId: 0 as PostId,
                blocks: args.blocks,
                astMap: postAst,
            }}
            skipCollapse={false}
            effectiveDate={args.publishedAt}
        />
    );

    const full = renderToString(
        <PostBody
            viewModel={{
                postId: 0 as PostId,
                blocks: args.blocks,
                astMap: postAst,
            }}
            skipCollapse={true}
            effectiveDate={args.publishedAt}
        />
    );

    return { preview, full };
}

export function renderMarkdown(args: MarkdownRenderRequest): MarkdownResult {
    const rendered = renderMarkdownReactNoHTML(args.markdown, new Date(), {
        renderingContext: "profile",
        hasCohostPlus: false,
        disableEmbeds: false,
        externalLinksInNewTab: false,
    });
    const html = renderToString(rendered);

    return { html };
}
