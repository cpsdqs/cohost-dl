import { CohostContext } from "./context.ts";
import { IPost } from "./model.ts";

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
