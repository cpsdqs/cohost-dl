import { CohostContext, encodeFilePathURI } from "./context.ts";
import { IPost, IProject } from "./model.ts";
import { rewriteMarkdownString } from "./markdown.ts";
import { GENERIC_OBSERVER } from "./config.ts";

interface IProfilePostsRes {
    pagination: {
        currentPage: number;
        morePagesForward: boolean;
        nextPage: number;
    };
    posts: IPost[];
}

export async function loadAllProjectPosts(
    ctx: CohostContext,
    projectHandle: string,
): Promise<IPost[]> {
    const trpcInput = (page: number) => ({
        projectHandle,
        page,
        options: {
            hideAsks: false,
            hideReplies: false,
            hideShares: false,
            pinnedPostsAtTop: true,
            viewingOnProjectPage: true,
        },
    });

    const posts: IPost[] = [];

    let page = 0;
    let hasNext = true;
    while (hasNext) {
        const response = await ctx.get(
            `https://cohost.org/api/v1/trpc/posts.profilePosts?batch=1&input=${
                encodeURIComponent(JSON.stringify({ "0": trpcInput(page) }))
            }`,
        );
        const res: {
            result: {
                data: IProfilePostsRes;
            };
        }[] = await response.json();

        hasNext = res[0].result.data.pagination.morePagesForward;
        page = res[0].result.data.pagination.nextPage;
        posts.push(...res[0].result.data.posts);

        // morePagesForward is wrong?
        if (!res[0].result.data.posts.length) break;
    }

    return posts;
}

export async function rewriteProject(
    ctx: CohostContext,
    project: IProject,
    base: string,
): Promise<Record<string, string>> {
    const rewrites: Record<string, string> = {};
    const fields: (
        | "avatarURL"
        | "avatarPreviewURL"
        | "headerURL"
        | "headerPreviewURL"
    )[] = [
        "avatarURL",
        "avatarPreviewURL",
        "headerURL",
        "headerPreviewURL",
    ];

    for (const field of fields) {
        if (project[field]) {
            const filePath = await ctx.loadResourceToFile(project[field]);
            if (filePath) {
                rewrites[project[field]] = encodeFilePathURI(base + filePath);
                project[field] = encodeFilePathURI(base + filePath);
            }
        }
    }

    const { markdown, urls } = await rewriteMarkdownString(
        ctx,
        project.description,
        base,
    );
    project.description = markdown;
    Object.assign(rewrites, urls);

    if (GENERIC_OBSERVER) {
        project.isSelfProject = null;
        project.contactCard = project.contactCard.filter(item => item.visibility !== "follows");
    }

    return rewrites;
}
