import { CohostContext } from "./context.ts";
import { getPageState, IPost } from "./model.ts";

interface ILikedPostsFeed {
    highlightedTags: string[];
    noPostsStringId: string;
    paginationMode: {
        currentSkip: number;
        idealPageStride: number;
        mode: 'refTimestampOffsetLimit';
        morePagesBackward: boolean;
        morePagesForward: boolean;
        pageUrlFactoryName: 'likedPosts';
        refTimestamp: number;
    };
    posts: IPost[];
}

export async function loadAllLikedPosts(ctx: CohostContext): Promise<IPost[]> {
    const URL = "https://cohost.org/rc/liked-posts";
    let refTimestamp: number | null = null;
    let skipPosts = 0;

    const posts: IPost[] = [];

    let hasNext = true;
    while (hasNext) {
        const params = new URLSearchParams();
        if (refTimestamp) params.set("refTimestamp", refTimestamp.toString());
        if (skipPosts) params.set("skipPosts", skipPosts.toString());

        const document = await ctx.getDocument(URL + '?' + params);

        const pageState = getPageState<ILikedPostsFeed>(
            document,
            "liked-posts-feed",
        );

        skipPosts += pageState.state.paginationMode.idealPageStride;
        refTimestamp = pageState.state.paginationMode.refTimestamp;
        hasNext = pageState.state.paginationMode.morePagesForward;

        posts.push(...pageState.state.posts);
    }

    return posts;
}
