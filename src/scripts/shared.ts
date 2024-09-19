export enum PostSearchFlags {
    AskResponse = 1,
    AdultContent = 2,
    Reply = 4,
    Share = 8,
    Pinned = 16,
    Editor = 32,
    Liked = 64,
}

export interface IPostSearchData {
    id: number;
    /** project handle */
    author: string;
    /** headline joined to body */
    contents: string;
    /** tags joined with \n */
    tags: string;
    /** date time string */
    published: string;
    flags: PostSearchFlags;
    /** chunk where this post can be found */
    chunk: string;
    /** if true, this post is the root of the post tree as stored in this chunk */
    isRoot: boolean;
}

export type IPostIndexedData = Pick<IPostSearchData, 'author' | 'tags' | 'published' | 'flags' | 'chunk'>;

export const PAGE_STRIDE = 20;
