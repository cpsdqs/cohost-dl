import { renderToString } from "react-dom/server";
import { renderMarkdownReactNoHTML } from "./cohost/lib/markdown/other-rendering";
import { generatePostAst } from "./cohost/lib/markdown/post-rendering";
import { PostBody } from "./cohost/preact/components/posts/post-body";
import { AttachmentRowViewBlock, AttachmentViewBlock, ViewBlock } from "./cohost/shared/types/post-blocks";
import { PostId } from "./cohost/shared/types/ids";
import { RenderingContext } from "./cohost/lib/markdown/shared-types";

const global = globalThis as Record<string, unknown>;

interface PostRenderRequest {
    blocks: ViewBlock[];
    publishedAt: string;
    hasCohostPlus: boolean;
    resources: string[];
}

interface PostResult {
    preview: string;
    full: string;
}

interface MarkdownRenderRequest {
    markdown: string;
    publishedAt: string;
    context: RenderingContext;
    hasCohostPlus: boolean;
    resources: string[];
}

interface MarkdownResult {
    html: string;
}

function makeResourceURL(url: string): string {
    return `/resource?url=${encodeURIComponent(url)}`;
}

function rewriteAttachment(attachment: AttachmentViewBlock, resources: string[]): AttachmentViewBlock {
    let inner = { ...attachment.attachment };

    if (resources.includes(inner.fileURL)) {
        inner.fileURL = makeResourceURL(inner.fileURL);
    }
    if (resources.includes(inner.previewURL)) {
        inner.previewURL = makeResourceURL(inner.previewURL);
    }

    // explicitly add type because we don't actually save that field in attachment rows
    return { ...attachment, type: "attachment", attachment: inner } as AttachmentViewBlock;
}

function rewriteAttachmentRow(attachmentRow: AttachmentRowViewBlock, resources: string[]): AttachmentRowViewBlock {
    return {
        type: "attachment-row",
        attachments: attachmentRow.attachments.map((item) => rewriteAttachment(item, resources)),
    };
}

export async function renderPost(args: PostRenderRequest): Promise<PostResult> {
    const blocks = [];
    for (const block of args.blocks) {
        if (block.type === "ask") {
            blocks.push(block);
        } else if (block.type === "attachment") {
            blocks.push(rewriteAttachment(block, args.resources));
        } else if (block.type === "attachment-row") {
            blocks.push(rewriteAttachmentRow(block, args.resources));
        } else if (block.type === "markdown") {
            blocks.push(block);
        } else {
            throw new Error("unexpected block type");
        }
    }

    // TODO: process post blocks & AST

    const postAst = await generatePostAst(blocks, new Date(args.publishedAt), {
        hasCohostPlus: args.hasCohostPlus,
        renderingContext: "post",
    });

    const preview = renderToString(
        <PostBody
            viewModel={{
                postId: 0 as PostId,
                blocks,
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
                blocks,
                astMap: postAst,
            }}
            skipCollapse={true}
            effectiveDate={args.publishedAt}
        />
    );

    return { preview, full };
}

export function renderMarkdown(args: MarkdownRenderRequest): MarkdownResult {
    const rendered = renderMarkdownReactNoHTML(args.markdown, new Date(args.publishedAt), {
        renderingContext: args.context,
        hasCohostPlus: args.hasCohostPlus,
        disableEmbeds: true,
        externalLinksInNewTab: false,
    });
    const html = renderToString(rendered);

    return { html };
}
