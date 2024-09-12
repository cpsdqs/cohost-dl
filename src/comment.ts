import { IComment } from "./model.ts";
import { CohostContext } from "./context.ts";
import { rewriteProject } from "./project.ts";
import { rewriteMarkdownString } from "./markdown.ts";

export async function rewriteComment(
    ctx: CohostContext,
    comment: IComment,
    base: string,
): Promise<Record<string, string>> {
    const rewrites: Record<string, string> = {};

    if (comment.poster) {
        Object.assign(
            rewrites,
            await rewriteProject(ctx, comment.poster, base),
        );
    }

    if (comment.comment?.body) {
        const { markdown, urls } = await rewriteMarkdownString(ctx, comment.comment.body, base);
        Object.assign(rewrites, urls);
        comment.comment.body = markdown;
    }

    for (const child of comment.comment?.children ?? []) {
        Object.assign(rewrites, await rewriteComment(ctx, child, base));
    }

    return rewrites;
}
