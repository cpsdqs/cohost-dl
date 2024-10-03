// TODO: replace with actual post renderer

/**
 * @typedef {{
 *     projectId: string;
 *     handle: string;
 *     avatarURL: string;
 *     avatarPreviewURL: string;
 *     privacy: string;
 *     flags: string[];
 *     avatarShape: string;
 *     displayName: string;
 * }} PostBlockAskProject
 * @typedef {{
 *     anon: boolean;
 *     loggedIn: boolean;
 *     askingProject: null | PostBlockAskProject
 *     askId: string;
 *     content: string;
 *     sentAt: string;
 * }} PostBlockAsk
 * @typedef {{
 *     kind: "image";
 *     altText: string | null;
 *     attachmentId: string | null;
 *     fileURL: string;
 *     previewURL: string;
 *     width: number | null;
 *     height: number | null;
 * }} PostBlockAttachmentImage
 * @typedef {{
 *     kind: "audio";
 *     artist: string | null;
 *     title: string | null;
 *     fileURL: string;
 *     previewURL: string;
 * }} PostBlockAttachmentAudio
 * @typedef {PostBlockAttachmentImage | PostBlockAttachmentAudio} PostBlockAttachment
 * @typedef {{
 *     attachments: { attachment: PostBlockAttachment }[]
 * }} PostBlockAttachments
 * @typedef {{
 *     content: string;
 * }} PostBlockMarkdown
 * @typedef {{
 *     type: "ask";
 *     ask: PostBlockAsk;
 * } | {
 *     type: "attachment";
 *     attachment: PostBlockAttachment;
 * } | {
 *     type: "attachment-row";
 *     attachments: PostBlockAttachments;
 * } | {
 *     type: "markdown";
 *     markdown: PostBlockMarkdown;
 * }} PostBlock
 */

/**
 * @param args {{
 *     blocks: PostBlock[],
 *     publishedAt: string,
 *     hasCohostPlus: boolean,
 *     disableEmbeds: boolean,
 *     externalLinksInNewTab: boolean,
 *     resources: Record<string, string>,
 * }}
 * @returns {{ html: string }}
 */
function renderPost(
    args,
) {
    /** @param block {PostBlock} */
    const basicRender = (block) => {
        if (block.type === "ask") {
            return `
                <blockquote>
                    <h3>this is an ask block</h3>
                    <div>
                        sent by
                        <img style="width: 2em; height: 2em; object-fit: cover" src="${block.ask.askingProject.avatarURL}" />
                        @${block.ask.askingProject.handle}
                    </div>
                    <div>
                        ${block.ask.content}
                    </div>
                </blockquote>
            `;
        } else if (block.type === "attachment") {
            if (block.attachment.kind === "image") {
                return `
                    <div>
                        <h3>this is an attachment block (image)</h3>
                        <img style="max-width: 400px; max-height: 400px" src="${block.attachment.fileURL}" alt="${block.attachment.altText}" />
                    </div>
                `;
            } else if (block.attachment.kind === "audio") {
                return `
                    <div>
                        <h3>this is an attachment block (audio)</h3>
                        <audio src="${block.attachment.fileURL}" />
                    </div>
                `;
            } else {
                throw new Error(`unknown attachment kind ${block.attachment.kind}`);
            }
        } else if (block.type === "attachment-row") {
            const contents = block.attachments.map(block => basicRender({ type: "attachment", ...block })).join("\n");
            return `
                <div>
                    <h3>attachment row!!!</h3>
                    ${contents}
                </div>
            `;
        } else if (block.type === "markdown") {
            return `
                <div>
                    <h3>markdown block</h3>
                    ${block.markdown.content}
                </div>
            `;
        }
        throw new Error(`unknown block type ${block.type}`);
    };

    return {
        html: args.blocks.map(basicRender).join("\n\n"),
    };
}

globalThis.renderPost = renderPost;
