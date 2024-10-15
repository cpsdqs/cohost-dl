import { renderToString } from "react-dom/server";
import { renderMarkdownReactNoHTML } from "./cohost/lib/markdown/other-rendering";
import { generatePostAst } from "./cohost/lib/markdown/post-rendering";
import { PostBodyInner } from "./cohost/preact/components/posts/post-body";
import {
    AskViewBlock,
    AttachmentRowViewBlock,
    AttachmentViewBlock,
    ViewBlock
} from "./cohost/shared/types/post-blocks";
import { PostId } from "./cohost/shared/types/ids";
import { RenderingContext } from "./cohost/lib/markdown/shared-types";
import { chooseAgeRuleset } from "./cohost/lib/markdown/sanitize";
import { WirePostViewModel } from "./cohost/shared/types/wire-models";
import { Element, Node, Parent, Root } from "hast";
import { Image, Node as MdastNode, Parent as MdastParent, Root as MdastRoot } from "mdast";
import { unified } from "unified";
import { Raw } from "mdast-util-to-hast";
import rehypeRaw from "rehype-raw";
import rehypeStringify from "rehype-stringify";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkStringify from "remark-stringify";
import { generate as cssGenerate, parse as cssParse, walk as cssWalk } from "css-tree";
import remarkBreaks from "remark-breaks";

interface PostRenderRequest {
    postId: number,
    blocks: ViewBlock[];
    publishedAt: string;
    hasCohostPlus: boolean;
    resources: string[];
}

interface PostResult {
    preview: string;
    full: string | null;
    className: string;
    viewModel: Pick<WirePostViewModel, "blocks" | "astMap" | "postId">;
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

function makeResourceURL(urlStr: string): string {
    let url: URL;
    try {
        url = new URL(urlStr);
    } catch {
        return `/r?url=${encodeURIComponent(urlStr)}`;
    }

    const search = new URLSearchParams();
    if (url.search) search.set('q', url.search.replace(/^\?/, ''));
    if (url.hash) search.set('h', url.hash.replace(/^#/, ''));

    const proto = url.protocol.replace(/:$/, '');

    return `/r/${proto}/${url.host}${url.pathname}${search.size ? `?${search}` : ''}`;
}

function rewriteMdastPlugin(resources: string[]) {
    const rewrite = (node: MdastNode) => {
        if (node.type === "image") {
            const image = node as Image;
            if (resources.includes(image.url)) {
                return { ...image, url: makeResourceURL(image.url) };
            }
        }

        if ("children" in node) {
            const parent = node as MdastParent;
            return { ...parent, children: parent.children.map(rewrite) }
        }

        return node;
    };

    return () => (tree: MdastRoot) => rewrite(tree) as MdastRoot;
}

function rewriteMarkdownString(markdown: string, resources: string[], date: Date): string {
    const ruleset = chooseAgeRuleset(date);

    let processor = unified().use(remarkParse);

    if (ruleset.singleLineBreaks) {
        processor = processor.use(remarkBreaks);
    }

    return processor
        .use(remarkGfm, { singleTilde: false })
        .use(rewriteMdastPlugin(resources))
        .use(remarkStringify)
        .processSync(markdown)
        .toString();
}

function rewriteAsk(ask: AskViewBlock, resources: string[]): AskViewBlock {
    const content = rewriteMarkdownString(ask.ask.content, resources, new Date(ask.ask.sentAt));

    if (!ask.ask.anon) {
        let askingProject = { ...ask.ask.askingProject };

        if (resources.includes(askingProject.avatarURL)) {
            askingProject.avatarURL = makeResourceURL(askingProject.avatarURL);
        }
        if (resources.includes(askingProject.avatarPreviewURL)) {
            askingProject.avatarPreviewURL = makeResourceURL(askingProject.avatarPreviewURL);
        }

        return { ...ask, ask: { ...ask.ask, askingProject, content } } as AskViewBlock;
    } else {
        // this is null, but zod wants it to be undefined
        ask.ask.askingProject = undefined;
    }

    return { ...ask, ask: { ...ask.ask, content } } as AskViewBlock;
}

function rewriteAttachment(attachment: AttachmentViewBlock, resources: string[]): AttachmentViewBlock {
    let inner = {
        ...attachment.attachment,
        // altText is apparently required, but some older posts contain null
        altText: attachment.attachment.altText ?? '',
        // this field was missing in an earlier version. it's not that important though
        attachmentId: attachment.attachment.attachmentId
            ?? attachment.attachment.fileURL.split('/').find(p => p.match(/^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i)),
    };

    if (inner.kind === "audio") {
        // must not be null
        inner.artist = inner.artist ?? '';
        inner.title = inner.title ?? '';
    }

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

function rewriteHast<N extends Node>(node: N, resources: string[]): N {
    if (node.type === "element") {
        const element = node as Node as Element;

        let properties = element.properties;
        if (typeof properties.style === "string") {
            const tree = cssParse(properties.style, { context: "declarationList" });

            let mutated = false;
            cssWalk(tree, (node) => {
                if (node.type === "Url") {
                    const resolved = new URL(node.value, "https://cohost.org/");
                    if (resources.includes(resolved.href)) {
                        node.value = makeResourceURL(resolved.href);
                        mutated = true;
                    }
                }
            });

            if (mutated) {
                properties = { ...properties, style: cssGenerate(tree) };
            }
        }

        if (typeof properties.src === "string") {
            const resolved = new URL(properties.src, "https://cohost.org/");
            if (resources.includes(resolved.href)) {
                properties = { ...properties, src: makeResourceURL(resolved.href) };
            }
        }

        if (typeof properties.srcset === "string") {
            const parts = properties.srcset.split(' ').map(part => {
                let resolved: URL;
                try {
                    resolved = new URL(part, "https://cohost.org/");
                } catch {
                    return part;
                }

                if (resources.includes(resolved.href)) {
                    return makeResourceURL(resolved.href);
                }
                return part;
            });

            properties = { ...properties, srcset: parts.join(' ') };
        }

        if (element.tagName === "CustomEmoji" && typeof properties.url === "string") {
            const resolved = new URL(properties.url, "https://cohost.org/");
            if (resolved.hostname === "cohost.org" && resolved.pathname.startsWith('/static')) {
                properties = { ...properties, url: resolved.pathname };
            }
        }

        if (properties.dataTestid == "mention" && typeof properties.href === "string") {
            properties = {
                ...properties,
                href: new URL(properties.href, "https://cohost.org").pathname,
            };
        }

        return {
            ...element,
            properties,
            children: element.children.map((child) => rewriteHast(child, resources)),
        } as Node as N;
    }

    if ("children" in node) {
        const parent = node as Parent;
        return {
            ...parent,
            children: parent.children.map((child) => rewriteHast(child, resources)),
        } as Node as N;
    }

    return node;
}

export async function renderPost(args: PostRenderRequest): Promise<PostResult> {
    const blocks = [];
    for (const block of args.blocks) {
        if (block.type === "ask") {
            blocks.push(rewriteAsk(block, args.resources));
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

    const postAst = await generatePostAst(blocks, new Date(args.publishedAt), {
        hasCohostPlus: args.hasCohostPlus,
        renderingContext: "post",
    });

    for (const span of postAst.spans) {
        let ast: Root = JSON.parse(span.ast);
        ast = rewriteHast(ast, args.resources);
        span.ast = JSON.stringify(ast);
    }

    const ruleset = chooseAgeRuleset(new Date(args.publishedAt));
    const hasReadMore = postAst.readMoreIndex !== null;

    const viewModel = {
        publishedAt: args.publishedAt,
        postId: args.postId as PostId,
        blocks,
        astMap: postAst,
    };

    const preview = renderToString(
        <PostBodyInner
            viewModel={viewModel}
            renderUntilBlockIndex={hasReadMore ? postAst.readMoreIndex : blocks.length}
            ruleset={ruleset}
        />
    );

    let full = null;
    if (hasReadMore) {
        full = renderToString(
            <PostBodyInner
                viewModel={viewModel}
                renderUntilBlockIndex={blocks.length}
                ruleset={ruleset}
            />
        );
    }

    return { preview, full, className: ruleset.className, viewModel: JSON.stringify(viewModel) };
}

function rewriteHastPlugin(resources: string[]) {
    return () => (tree: Root) => rewriteHast(tree, resources);
}

export function renderMarkdown(args: MarkdownRenderRequest): MarkdownResult {
    const rendered = renderMarkdownReactNoHTML(args.markdown, new Date(args.publishedAt), {
        renderingContext: args.context,
        hasCohostPlus: args.hasCohostPlus,
        disableEmbeds: true,
        externalLinksInNewTab: false,
    });
    const html = renderToString(rendered);

    // this is inefficient, but it's probably fine
    const rewrittenTree = unified()
        .use(rehypeRaw)
        .use(rewriteHastPlugin(args.resources))
        .runSync({
            type: "root",
            children: [
                { type: "raw", value: html } as Raw,
            ],
        } as Root) as Root;

    const html2 = unified()
        .use(rehypeStringify)
        .stringify(rewrittenTree);

    return { html: html2 };
}
