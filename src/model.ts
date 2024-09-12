import { Document } from "jsr:@b-fuze/deno-dom";

interface IContactCardItem {
    value: string;
    service: string;
    visibility: "public" | "logged-in" | "follows";
}

enum ProjectFlag {
    StaffMember = "staffMember",
    FriendOfTheSite = "friendOfTheSite",
}

export interface IProject {
    askSettings: {
        enabled: boolean;
        allowAnon: boolean;
        requiredLoggedInAnon: boolean;
    };
    avatarPreviewURL: string;
    avatarShape: string;
    avatarURL: string;
    contactCard: IContactCardItem[];
    dek: string;
    deleteAfter: unknown;
    description: string;
    displayName: string;
    flags: ProjectFlag[];
    frequentlyUsedTags: string[];
    handle: string;
    headerPreviewURL: string;
    headerURL: string;
    isSelfProject: unknown;
    loggedOutPostVisibility: "public" | "none";
    privacy: "public" | "private";
    projectId: number;
    pronouns: string;
    url: string;
}

export type ISmallProject = Pick<
    IProject,
    | "avatarPreviewURL"
    | "avatarShape"
    | "avatarURL"
    | "displayName"
    | "flags"
    | "handle"
    | "privacy"
    | "projectId"
>;

export interface IPostBlockMarkdown {
    type: "markdown";
    markdown: {
        content: string;
    };
}

export interface IPostBlockAsk {
    type: "ask";
    ask: {
        anon: boolean;
        askId: string;
        askingProject: ISmallProject;
        content: string;
        loggedIn: boolean;
        sentAt: string;
    };
}

export interface IAttachmentImage {
    altText: string;
    attachmentId: string;
    fileURL: string;
    height: string;
    kind: "image";
    previewURL: string;
    width: string;
}

export interface IAttachmentAudio {
    artist: string;
    attachmentId: string;
    fileURL: string;
    kind: "audio";
    previewURL: string;
    title: string;
}

export type IAttachment = IAttachmentImage | IAttachmentAudio;

export interface IPostBlockAttachment {
    type: "attachment";
    attachment: IAttachment;
}

export type IPostBlock =
    | IPostBlockAttachment
    | IPostBlockAsk
    | IPostBlockMarkdown;

export interface IPost {
    astMap: {
        readMoreIndex: number;
        spans: {
            ast: string;
            startIndex: number;
            endIndex: number;
        }[];
    };
    blocks: IPostBlock[];
    canPublish: boolean;
    canShare: boolean;
    commentsLocked: boolean;
    cws: string[];
    effectiveAdultContent: boolean;
    filename: string;
    hasAnyContributorMuted: boolean;
    hasCohostPlus: boolean;
    headline: string;
    isEditor: boolean;
    isLiked: boolean;
    limitedVisibilityReason: string;
    numComments: number;
    numSharedComments: number;
    pinned: boolean;
    plainTextBody: string;
    postEditUrl: string;
    postId: number;
    postingProject: IProject;
    publishedAt: string;
    relatedProjects: IProject[];
    responseToAskId: string | null;
    shareOfPostId: number | null;
    shareTree: IPost[];
    sharesLocked: boolean;
    singlePostPageUrl: string;
    state: number;
    tags: string[];
    transparentShareOfPostId: string | null;
}

export interface ITRPCQuery {
    queryHash: string;
    queryKey: [string[], {
        input?: object;
        type: "query";
    }];
    state: {
        data: object;
        error: unknown | null;
        status: "success" | string;
    };
}

function jsonEq(a: unknown, b: unknown): boolean {
    if (a === undefined && b === undefined) return true;
    if (a === null && b === null) return true;
    if (["boolean", "number", "string"].includes(typeof a)) {
        return a === b;
    }
    if (Array.isArray(a)) {
        if (!Array.isArray(b)) return false;
        if (a.length !== b.length) return false;
        for (let i = 0; i < a.length; i++) {
            if (!jsonEq(a[i], b[i])) return false;
        }
        return true;
    }
    if (typeof a === "object") {
        if (typeof b !== "object") return false;
        const aKeys = Object.keys(a as object).sort();
        const bKeys = Object.keys(b as object).sort();
        if (!jsonEq(aKeys, bKeys)) return false;
        type O = Record<string, unknown>;
        for (const k in a) if (!jsonEq((a as O)[k], (b as O)[k])) return false;
        return true;
    }

    return false;
}

export class PageState<S> {
    state: S;
    trpcQueries: ITRPCQuery[];

    constructor(state: S, trpcQueries: ITRPCQuery[]) {
        this.state = state;
        this.trpcQueries = trpcQueries;
    }

    query<T>(query: string, input?: object) {
        const state = this.trpcQueries.find((item) =>
            item.queryKey[0].join(".") === query &&
            jsonEq(input, item.queryKey[1].input)
        )?.state;
        if (!state) {
            throw new Error(
                `query not found: ${query} ${JSON.stringify(input)}`,
            );
        }
        if (state.status !== "success") {
            throw new Error(
                `query ${query} ${JSON.stringify(input)} not successful`,
            );
        }
        return state.data as T;
    }
}

export function getPageState<S>(
    document: Document,
    stateName?: string,
): PageState<S> {
    const state = JSON.parse(
        document.querySelector("script#__COHOST_LOADER_STATE__")?.innerHTML ??
            "",
    );
    const trpcState = JSON.parse(
        document.querySelector("script#trpc-dehydrated-state")?.innerHTML ?? "",
    );

    return new PageState<S>(
        stateName ? state[stateName] : state,
        trpcState.queries,
    );
}