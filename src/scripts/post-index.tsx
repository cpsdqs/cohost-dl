import React, {
    MouseEvent,
    ReactNode,
    useCallback,
    useEffect,
    useRef,
    useState,
} from "react";
import ReactDOM from "react-dom/client";
import {
    Hydrate,
    QueryClient,
    QueryClientProvider,
} from "@tanstack/react-query";
import {
    EyeIcon,
    EyeSlashIcon,
    MagnifyingGlassIcon,
} from "@heroicons/react/24/outline";
import {
    ChevronDownIcon,
    ChevronLeftIcon,
    ChevronRightIcon,
    HashtagIcon,
} from "@heroicons/react/24/solid";
import { useSSR } from "react-i18next";
import { httpBatchLink } from "@trpc/client/links/httpBatchLink";
import sitemap from "@/shared/sitemap";
import { trpc } from "@/lib/trpc";
import { ProfileView } from "@/preact/components/partials/profile-view";
import { PostPreview } from "@/preact/components/posts/post-preview";
import { useDisplayPrefs } from "@/preact/hooks/use-display-prefs";
import { UserInfoContext } from "@/preact/providers/user-info-provider";
import { LightboxHost } from "@/preact/components/lightbox";
import { CohostLogo } from "@/preact/components/elements/icon";
import { PaginationEggs } from "@/preact/components/partials/pagination-eggs";
import { Loading } from "@/preact/components/loading";
import { TokenInput } from "@/preact/components/token-input";
import MiniSearch from "@internal/minisearch";
import { IPostIndexedData, PAGE_STRIDE, PostSearchFlags } from "./shared.ts";
import type { IPost, IProject } from "../model.ts";

const { project, rewriteData, chunks, searchTreeIndex, trpcState } = JSON
    .parse(
        document.querySelector("#post-index-data").innerHTML,
    ) as {
        project?: IProject;
        rewriteData?: { base: string; urls: Record<string, string> };
        chunks?: Record<string, string>;
        searchTreeIndex: Record<string, number[]>;
        trpcState: object;
    };

const allPostIds = [
    ...new Set(Object.values(searchTreeIndex).flatMap((id) => id)),
].sort((a, b) => b - a);

const allProjects = [
    ...new Set(
        Object.values(chunks ?? {})
            .map((chunk) => chunk.split("~")[0]),
    ),
].sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));

const postListChunkPromises = new Map<string, {
    promise: Promise<IPost[]>;
    resolve: (posts: IPost[]) => void;
}>();

let postSearchIndexPromise: null | {
    promise: Promise<MiniSearch>;
    resolve: (search: MiniSearch) => void;
    reject: (error: Error) => void;
} = null;

window.cohostDL = {
    project,
    rewriteData: rewriteData ??
        { base: "https://cohost.org/a/post/a", urls: {} },
    postSearchIndex(data: string) {
        MiniSearch.loadJSONAsync(data, {
            idField: "id",
            fields: ["author", "contents", "tags"],
            storeFields: ["author", "published", "flags", "chunk"],
        }).then((result: MiniSearch<IPostIndexedData>) => {
            postSearchIndexPromise?.resolve(result);
        }).catch((error: Error) => {
            postSearchIndexPromise?.reject(error);
        });
    },
    postListChunk(
        id: string,
        items: IPost[],
        newRwData: Record<string, string>,
    ) {
        Object.assign(window.cohostDL.rewriteData, newRwData);
        postListChunkPromises.get(id)!.resolve(items);
    },
};

function loadPostSearchIndex(): Promise<MiniSearch<IPostIndexedData>> {
    if (postSearchIndexPromise) return postSearchIndexPromise.promise;

    postSearchIndexPromise = {
        promise: null as unknown as Promise<unknown>,
        resolve: () => null,
        reject: () => null,
    };
    const promise = new Promise<MiniSearch<IPostIndexedData>>(
        (resolve, reject) => {
            const script = document.createElement("script");
            script.src = "cdl-index.js";
            script.addEventListener(
                "error",
                () => reject(new Error(`failed to load search index`)),
            );
            document.head.append(script);
            postSearchIndexPromise!.resolve = resolve;
            postSearchIndexPromise!.reject = reject;
        },
    );
    postSearchIndexPromise.promise = promise;
    return promise;
}

function loadPostListChunk(id: string): Promise<IPost[]> {
    const existingEntry = postListChunkPromises.get(id);
    if (existingEntry) return existingEntry.promise;

    const project = id.split("~")[0];

    const entry = {
        promise: null as unknown as Promise<IPost[]>,
        resolve: (() => null) as ((items: IPost[]) => void),
    };
    const promise = new Promise<IPost[]>((resolve, reject) => {
        const script = document.createElement("script");
        script.src = `../${project}/cdl-chunk~${id}.js`;
        script.addEventListener(
            "error",
            () => reject(new Error(`failed to load chunk ${id}`)),
        );
        document.head.append(script);
        entry.resolve = resolve;
    });
    postListChunkPromises.set(id, entry);
    return promise;
}

const ALL_VISIBLE = {
    canRead: "allowed",
};
const NO_TAGS: string[] = [];

const GENERIC_OBSERVER = {
    loggedIn: true,
    userId: 1,
    email: "cohost-dl@localhost",
    projectId: 1,
    modMode: false,
    activated: true,
    readOnly: false,
    emailVerifyCanceled: false,
    emailVerified: true,
    twoFactorActive: true,
    deleteAfter: null,
};

interface IPostFilter {
    tags: string[];
    ask: null | boolean;
    adult: null | boolean;
    reply: null | boolean;
    share: null | boolean;
    pinned: null | boolean;
    editor: null | boolean;
    liked: null | boolean;
}

const FLAG_FILTERS: (keyof IPostFilter)[] = [
    "liked",
    "ask",
    "reply",
    "share",
    "adult",
    "pinned",
    "editor",
];

const FLAG_FILTER_LABELS: Partial<Record<keyof IPostFilter, string>> = {
    liked: "liked",
    ask: "asks",
    reply: "replies",
    share: "shares",
    adult: "adult content",
    pinned: "pinned",
    editor: "editable",
};

function postIndexedDataFilter(item: IPostIndexedData, filter: IPostFilter) {
    const flagFilter = (filter: null | boolean, flag: PostSearchFlags) => {
        if (filter !== null) {
            const hasFlag = (item.flags & flag) !== 0;
            return hasFlag !== filter;
        }
        return false;
    };

    if (flagFilter(filter.ask, PostSearchFlags.AskResponse)) return false;
    if (flagFilter(filter.adult, PostSearchFlags.AdultContent)) return false;
    if (flagFilter(filter.reply, PostSearchFlags.Reply)) return false;
    if (flagFilter(filter.share, PostSearchFlags.Share)) return false;
    if (flagFilter(filter.pinned, PostSearchFlags.Pinned)) return false;
    if (flagFilter(filter.editor, PostSearchFlags.Editor)) return false;
    if (flagFilter(filter.liked, PostSearchFlags.Liked)) return false;

    if (filter.tags.length) {
        const postTags = item.tags.split("\n");
        if (filter.tags.some((tag) => !postTags.includes(tag))) return false;
    }

    return true;
}

function shouldFilterOnlyUseTreePosts(filter: IPostFilter) {
    const isFlagNonZero = (flag: null | boolean) => flag !== null;

    return isFlagNonZero(filter.share) || isFlagNonZero(filter.reply);
}

const POST_FILTER_NONE: IPostFilter = {
    tags: [],
    ask: null,
    adult: null,
    reply: null,
    share: null,
    pinned: null,
    editor: null,
    liked: null,
};

function deepEq(a: unknown, b: unknown) {
    if (Array.isArray(a) && Array.isArray(b)) {
        if (a.length !== b.length) return false;
        for (let i = 0; i < a.length; i++) {
            if (!deepEq(a[i], b[i])) return false;
        }
        return true;
    } else if (a && typeof a === "object" && b && typeof b === "object") {
        const keys = Object.keys(a).sort();
        const keysB = Object.keys(b).sort();
        if (!deepEq(keys, keysB)) return false;
        for (const k of keys) {
            if (
                !deepEq(
                    (a as Record<string, unknown>)[k],
                    (b as Record<string, unknown>)[k],
                )
            ) return false;
        }
        return true;
    }
    return a === b;
}

interface IPostSearch {
    page: number;
    query: string;
    filter: IPostFilter;
}

function useSearchIndex(use: boolean): {
    isLoading: boolean;
    error: Error | null;
    searchIndex: MiniSearch<IPostIndexedData> | null;
} {
    const [usedOnce, setUsedOnce] = useState(use);

    useEffect(() => {
        if (use) setUsedOnce(true);
    }, [use]);

    const [isLoading, setLoading] = useState(false);
    const [error, setError] = useState<Error | null>(null);
    const [searchIndex, setSearchIndex] = useState<
        MiniSearch<IPostIndexedData> | null
    >(null);

    useEffect(() => {
        if (usedOnce) {
            setLoading(true);
            loadPostSearchIndex().then(setSearchIndex).catch(setError).finally(
                () => setLoading(false),
            );
        }
    }, [usedOnce]);

    return { isLoading, error, searchIndex };
}

function usePostListChunks(requestedChunks: string[]): {
    loading: Set<string>;
    errors: Map<string, Error>;
    chunks: Map<string, IPost[]>;
    index: Map<number, [string, number]>;
} {
    const [loading, setLoading] = useState(new Set<string>());
    const [errors, setErrors] = useState(new Map<string, Error>());
    const [chunks, setChunks] = useState(new Map<string, IPost[]>());
    const [index, setIndex] = useState(new Map<number, [string, number]>());

    const chunkRequests = useRef(new Set<string>());

    useEffect(() => {
        for (const chunk of requestedChunks) {
            if (chunkRequests.current.has(chunk)) continue;
            chunkRequests.current.add(chunk);

            setLoading((loading) => {
                const newLoading = new Set(loading);
                newLoading.add(chunk);
                return newLoading;
            });

            loadPostListChunk(chunk).then((posts) => {
                setChunks((chunks) => {
                    const newChunks = new Map(chunks);
                    newChunks.set(chunk, posts);
                    return newChunks;
                });
                setIndex((index) => {
                    const newIndex = new Map(index);
                    for (let i = 0; i < posts.length; i++) {
                        newIndex.set(posts[i].postId, [chunk, i]);
                    }
                    return newIndex;
                });
            }).catch((error) => {
                setErrors((errors) => {
                    const newErrors = new Map(errors);
                    newErrors.set(chunk, error);
                    return newErrors;
                });
            }).finally(() => {
                setLoading((loading) => {
                    const newLoading = new Set(loading);
                    newLoading.delete(chunk);
                    return newLoading;
                });
            });
        }
    }, [requestedChunks]);

    return {
        loading,
        errors,
        chunks,
        index,
    };
}

const globalChunks = chunks;
function useFilteredPosts(search: IPostSearch) {
    const useSearch = !!search.query ||
        !deepEq(search.filter, POST_FILTER_NONE);

    const {
        isLoading: searchIndexLoading,
        error: searchIndexError,
        searchIndex,
    } = useSearchIndex(useSearch);

    let isLoading = false;
    const errors: Error[] = [];

    let posts: number[] = [];
    let maxPage = 0;

    let requestedChunks: string[] = [];

    const start = search.page * PAGE_STRIDE;
    const end = (search.page + 1) * PAGE_STRIDE;

    if (useSearch) {
        if (searchIndex) {
            const onlyUseTreePosts = shouldFilterOnlyUseTreePosts(
                search.filter,
            );

            const results = searchIndex.search(
                search.query || MiniSearch.wildcard,
                {
                    fields: ["author", "contents"],
                    fuzzy: (token) => token.length > 5 ? 0.1 : 0,
                    prefix: true,
                    filter: (item: IPostIndexedData) =>
                        postIndexedDataFilter(item, search.filter),
                    combineWith: "AND",
                },
            );

            const treeResults: { id: number; chunk: string }[] = [];
            const includedPosts = new Set<number>();

            for (const item of results) {
                const post = item.id;
                const treePosts = searchTreeIndex[post] ?? [];

                if (treePosts.includes(post)) {
                    if (!includedPosts.has(post)) {
                        treeResults.push(item);
                        includedPosts.add(post);
                    }
                    continue;
                } else if (onlyUseTreePosts) {
                    continue;
                }

                const alreadyIncluded = treePosts.some((post) =>
                    includedPosts.has(post)
                );
                if (!alreadyIncluded) {
                    treeResults.push({ id: treePosts[0], chunk: item.chunk });
                    includedPosts.add(treePosts[0]);
                }
            }

            const searchResults = treeResults.slice(start, end);
            posts = searchResults.map((result) => result.id);
            requestedChunks = searchResults.map((result) => result.chunk);

            maxPage = Math.max(
                0,
                Math.floor((treeResults.length - 1) / PAGE_STRIDE),
            );
        } else {
            isLoading = isLoading || searchIndexLoading;
            if (searchIndexError) errors.push(searchIndexError);
        }
    } else {
        posts = allPostIds.slice(start, end);

        if (project) {
            // FIXME: really unreliable
            requestedChunks = [`${project.handle}~${search.page}`];
        } else {
            for (const post of posts) {
                const chunk = globalChunks[post];
                if (chunk && !requestedChunks.includes(chunk)) {
                    requestedChunks.push(chunk);
                }
            }
        }

        maxPage = Math.floor((allPostIds.length - 1) / PAGE_STRIDE);
    }

    const {
        loading: loadingChunks,
        errors: chunkErrors,
        chunks,
        index,
    } = usePostListChunks(requestedChunks);

    for (const chunk of requestedChunks) {
        if (loadingChunks.has(chunk)) isLoading = true;
        const error = chunkErrors.get(chunk);
        if (error) errors.push(error);
    }

    const postData = posts.map((post) => {
        const loc = index.get(post);
        if (loc) {
            return chunks.get(loc[0])?.[loc[1]];
        }
    }).filter((x) => x) as IPost[];

    return {
        isLoading,
        errors,
        maxPage,
        posts: postData,
    };
}

// for syntax highlighting
const css = (x: TemplateStringsArray) => x.join("");

const cdlStyles = css`
    :root {
        --cdl-bg: 255 249 242;
        --cdl-fg: 0 0 0;
        --cdl-shadow: 0px 4px 5px rgba(0, 0, 0, .14), 0px 1px 10px rgba(0, 0, 0, .12), 0px 2px 4px rgba(0, 0, 0, .2);

        --cdl-tag-bg: 131 37 79;
        --cdl-tag-fg: 255 249 242;

        --cdl-accent: 131 37 79;
    }

    @media (prefers-color-scheme: dark) {
        :root {
            --cdl-bg: 25 25 25;
            --cdl-outline: 127 127 127;
            --cdl-fg: 240 240 240;
            --cdl-shadow: 0 0 0 1px #fff3, 0px 4px 5px rgba(0, 0, 0, .14), 0px 1px 10px rgba(0, 0, 0, .12), 0px 2px 4px rgba(0, 0, 0, .2);

            --cdl-tag-bg: 229 143 62;
            --cdl-tag-fg: 25 25 25;

            --cdl-accent: 229 143 62;
        }
    }

    .cdl-search-box-container {
        padding-left: 5.5rem;

        &.is-condensed {
            padding-left: 0;
        }
    }

    .cdl-search-box {
        margin-top: 1rem;
        margin-bottom: 1rem;
        background: rgb(var(--cdl-bg));
        color: rgb(var(--cdl-fg));
        box-shadow: var(--cdl-shadow);
        grid-template-columns: auto auto;
        border-radius: 0.5rem;
        display: grid;
        position: relative;

        > .i-search {
            display: grid;
            grid-column: 1 / 3;
            grid-template-columns: auto 1fr;
            align-items: center;
            border-radius: 0.5rem;
            padding-left: 0.5rem;

            > .i-search-icon {
                width: 1.1rem;
            }

            > .i-search-field {
                margin: 0;
                background: none;
                appearance: none;
                font: inherit;
                color: inherit;
                border: none;
                padding: 0.25rem 0.5rem;

                &:focus {
                    outline: none;
                    box-shadow: none;
                }
            }

            &:focus-within {
                box-shadow: 0 0 0 0.25rem rgb(var(--cdl-accent));
            }
        }

        > .i-tags {
            grid-column: 1 / 3;
            padding: 0.25rem 0;

            .co-filled-button {
                background: rgb(var(--cdl-tag-bg));
                color: rgb(var(--cdl-tag-fg));
            }
        }

        > .i-flag-filters {
            > .i-button {
                display: flex;
                flex-wrap: wrap;
                gap: 0.25rem 0.5rem;
                padding: 0 0.5rem;
                transition: opacity 0.2s;

                &:active {
                    opacity: 0.5;
                    transition: none;
                }

                > .i-flag,
                > .i-no-flags {
                    display: grid;
                    grid-template-columns: auto auto;
                    align-items: center;
                    gap: 0.25rem;

                    > .i-icon {
                        height: 1rem;
                    }
                }
            }

            > .i-settings {
                animation: cdl-flag-filter-settings-in 0.2s;
                position: absolute;
                max-width: 100%;
                background: rgb(var(--cdl-bg));
                color: rgb(var(--cdl-fg));
                box-shadow: var(--cdl-shadow);
                border-radius: 0.75rem;
                z-index: 10;
                display: grid;
                grid-template-columns: auto auto;
                gap: 0.25rem 0.5rem;
                padding: 0.5rem;

                > .i-item {
                    display: contents;

                    > .i-segmented {
                        display: grid;
                        background: rgb(var(--cdl-fg) / 0.1);
                        grid-template-columns: 1fr 1fr 1fr;
                        border-radius: 0.25rem;

                        > button {
                            padding: 0 0.25rem;
                            border: 1px solid transparent;

                            &:not(.is-active) + button:not(.is-active) {
                                border-left: 1px solid rgb(var(--cdl-fg) / 0.1);
                            }

                            &.is-active {
                                border-radius: 0.25rem;
                                background: rgb(var(--cdl-tag-bg));
                                color: rgb(var(--cdl-tag-fg));
                            }
                        }
                    }
                }
            }
        }

        > .i-pagination {
            display: grid;
            place-self: end;
            grid-template-columns: auto auto auto;

            > .i-page-button {
                width: 1.5rem;
                height: 1.5rem;
                display: grid;
                place-content: center;
                transition: opacity 0.2s;

                &:disabled {
                    opacity: 0.5;
                }

                &:active {
                    opacity: 0.5;
                    transition: none;
                }

                > .i-icon {
                    width: 1.1rem;
                }
            }

            > .i-page {
                min-width: 3em;
                font-variant-numeric: tabular-nums;
                text-align: center;

                &.is-editor {
                    background: none;
                    padding: 0;
                    width: 6em;
                    background: none;
                    color: inherit;
                    border-radius: 0.25rem;
                }
            }
        }
    }

    .cdl-loading-container {
        display: grid;
        place-content: center;
        padding: 1rem;
        color: rgb(var(--cdl-fg));
    }

    .cdl-main {
        width: 100%;
        max-width: 80em;
        margin: 0 auto;
        display: grid;
        grid-template-columns: 1fr 2fr 1fr;
        gap: 2rem;

        > .cdl-all-projects {
            background: rgb(var(--cdl-bg));
            color: rgb(var(--cdl-fg));
            box-shadow: var(--cdl-shadow);
            margin: 1rem;
            border-radius: 0.5rem;
            min-width: 0;
            max-height: calc(100svh - 6rem);
            display: grid;

            > h3 {
                padding: 0.5rem 0.75rem;
                font-size: 1.2rem;
                font-weight: bold;
            }

            > ul {
                overflow: hidden auto;
            }

            > ul > li > a {
                display: block;
                padding: 0 0.75rem;
                white-space: nowrap;
                overflow: hidden;
                text-overflow: ellipsis;

                &:hover {
                    text-decoration: underline;
                }
            }
        }

        > .cdl-all-posts {
            min-width: 0;
        }
    }

    @media (max-width: 1023px) {
        .cdl-main {
            grid-template-columns: 1fr 2fr 0;
        }
    }

    @media (max-width: 769px) {
        .cdl-main {
            grid-template-columns: 1fr;

            > .cdl-all-projects {
                grid-row: 2;
            }
        }
    }

    @keyframes cdl-flag-filter-settings-in {
        from {
            opacity: 0
        }
    }
`;

{
    const style = document.createElement("style");
    style.innerHTML = cdlStyles;
    document.head.append(style);
}

function FlagFilterSettings(
    { filter, onChange }: {
        filter: IPostFilter;
        onChange: (filter: Partial<IPostFilter>) => void;
    },
) {
    return (
        <>
            {FLAG_FILTERS.map((flag) => (
                <div className="i-item" key={flag}>
                    <div className="i-label">
                        {FLAG_FILTER_LABELS[flag]}
                    </div>
                    <div className="i-segmented">
                        <button
                            className={filter[flag] === null ? "is-active" : ""}
                            onClick={() => onChange({ [flag]: null })}
                        >
                            show
                        </button>
                        <button
                            className={filter[flag] === false
                                ? "is-active"
                                : ""}
                            onClick={() => onChange({ [flag]: false })}
                        >
                            hide
                        </button>
                        <button
                            className={filter[flag] === true ? "is-active" : ""}
                            onClick={() => onChange({ [flag]: true })}
                        >
                            only
                        </button>
                    </div>
                </div>
            ))}
        </>
    );
}

function SearchBox({ search, onChange, maxPage }: {
    search: IPostSearch;
    onChange: (search: Partial<IPostSearch>) => void;
    maxPage: number;
}) {
    const onFilterChange = (filter: Partial<IPostFilter>) =>
        onChange({ filter: { ...search.filter, ...filter } });

    const [query, setQuery] = useState(search.query);
    const [isSearchFocused, setSearchFocused] = useState(false);

    const debouncedQuery = useDebounced(query);
    useEffect(() => {
        if (!isSearchFocused) {
            onChange({ query });
        } else {
            onChange({ query: debouncedQuery });
        }
    }, [isSearchFocused, debouncedQuery]);

    const [wantsSearchIndex, setWantsSearchIndex] = useState(false);
    const { searchIndex } = useSearchIndex(wantsSearchIndex);

    const onTagSearch = (query: string) => {
        if (!searchIndex) {
            if (query) {
                setTimeout(() => {
                    setWantsSearchIndex(true);
                }, 10);
            }
            return { mappedSuggestions: [] };
        }

        const results = searchIndex.search(query ?? MiniSearch.wildcard, {
            fields: ["tags"],
            prefix: true,
        });

        return {
            mappedSuggestions: [
                ...new Set(results.flatMap((item) => item.tags.split("\n"))),
            ]
                .filter((tag) =>
                    tag.toLowerCase().startsWith(query.toLowerCase())
                )
                .slice(0, 10),
        };
    };

    const [flagFiltersOpen, setFlagFiltersOpen] = useState(false);
    const flagFiltersNode = useRef<HTMLDivElement>(null);

    useEffect(() => {
        // close flag filters when interacting with something that isn't flag filters
        const onInteract = (e: UIEvent) => {
            if (!flagFiltersNode.current) return;
            if (!flagFiltersNode.current.contains(e.target)) {
                setFlagFiltersOpen(false);
            }
        };

        window.addEventListener("pointerdown", onInteract);
        window.addEventListener("keydown", onInteract);
        window.addEventListener("wheel", onInteract);
        return () => {
            window.removeEventListener("pointerdown", onInteract);
            window.removeEventListener("keydown", onInteract);
            window.removeEventListener("wheel", onInteract);
        };
    }, []);

    const flagFiltersPreview: ReactNode[] = [];
    for (const flag of FLAG_FILTERS) {
        if (search.filter[flag] !== null) {
            flagFiltersPreview.push(
                <span className="i-flag" key={flag}>
                    {search.filter[flag]
                        ? <EyeIcon className="i-icon" />
                        : <EyeSlashIcon className="i-icon" />}
                    <span className="i-label">
                        {FLAG_FILTER_LABELS[flag]}
                    </span>
                </span>,
            );
        }
    }
    if (!flagFiltersPreview.length) {
        flagFiltersPreview.push(
            <span className="i-no-flags" key="[]">
                <span className="i-label">filters</span>
                <ChevronDownIcon className="i-icon" />
            </span>,
        );
    }

    const [isEditingPage, setEditingPage] = useState(false);

    return (
        <div className="cdl-search-box">
            <div className="i-search">
                <MagnifyingGlassIcon className="i-search-icon" />
                <input
                    className="i-search-field"
                    placeholder="Search"
                    type="search"
                    value={query}
                    onFocus={() => setSearchFocused(true)}
                    onBlur={() => setSearchFocused(false)}
                    onChange={(e) => setQuery(e.target.value)}
                />
            </div>
            <div className="i-tags">
                <TokenInput
                    className="co-editable-body w-full p-0 px-2 leading-none"
                    TokenIcon={HashtagIcon}
                    tokens={search.filter.tags}
                    setTokens={(tags) => onFilterChange({ tags })}
                    placeholder="search tags"
                    getSuggestions
                    onTagSearch={onTagSearch}
                />
            </div>
            <div className="i-flag-filters" ref={flagFiltersNode}>
                <button
                    className="i-button"
                    onClick={() => setFlagFiltersOpen((open) => !open)}
                >
                    {flagFiltersPreview}
                </button>
                {flagFiltersOpen
                    ? (
                        <div className="i-settings">
                            <FlagFilterSettings
                                filter={search.filter}
                                onChange={onFilterChange}
                            />
                        </div>
                    )
                    : null}
            </div>
            <div className="i-pagination">
                <button
                    className="i-page-button"
                    type="button"
                    aria-label="Previous page"
                    disabled={!search.page}
                    onClick={() => {
                        onChange({ page: search.page - 1 });
                    }}
                >
                    <ChevronLeftIcon className="i-icon" />
                </button>
                {isEditingPage
                    ? (
                        <input
                            autoFocus
                            className="i-page is-editor"
                            type="number"
                            defaultValue={search.page + 1}
                            onKeyDown={(e) => {
                                if (e.key === "Enter") e.target.blur();
                            }}
                            onFocus={(e) => {
                                e.target.select();
                            }}
                            onBlur={(e) => {
                                const page = parseInt(e.target.value, 10);
                                if (Number.isFinite(page)) {
                                    onChange({
                                        page: Math.min(
                                            Math.max(0, page - 1),
                                            maxPage,
                                        ),
                                    });
                                }
                                setEditingPage(false);
                            }}
                        />
                    )
                    : (
                        <button
                            type="button"
                            className="i-page"
                            onClick={() => setEditingPage(true)}
                        >
                            {search.page + 1}/{maxPage + 1}
                        </button>
                    )}
                <button
                    className="i-page-button"
                    type="button"
                    aria-label="Next page"
                    disabled={search.page >= maxPage}
                    onClick={() => {
                        onChange({ page: search.page + 1 });
                    }}
                >
                    <ChevronRightIcon className="i-icon" />
                </button>
            </div>
        </div>
    );
}

function useDebounced<T>(value: T): T {
    const [debounced, setDebounced] = useState(value);

    const currentValue = useRef(value);
    currentValue.current = value;

    const changeTimeout = useRef<null | number>(null);
    useEffect(() => {
        if (changeTimeout.current) return;
        changeTimeout.current = setTimeout(() => {
            changeTimeout.current = null;
            setDebounced(currentValue.current);
        }, 1000);
    }, [value]);

    useEffect(() => {
        return () => {
            if (changeTimeout.current) clearTimeout(changeTimeout.current);
        };
    }, []);

    return debounced;
}

function FilteredPostFeed({ condensed }: { condensed?: boolean }) {
    const displayPrefs = useDisplayPrefs();
    const [search, setSearch] = useState({
        page: 0,
        query: "",
        filter: POST_FILTER_NONE,
    });

    const { isLoading, errors, posts, maxPage } = useFilteredPosts(search);
    const hasNextPage = search.page < maxPage;

    const postListTop = useRef<HTMLDivElement>(null);

    const onPageBack = useCallback((e: MouseEvent) => {
        e.preventDefault();
        setSearch((search) => ({
            ...search,
            page: Math.max(0, search.page - 1),
        }));

        postListTop.current?.scrollIntoView({
            block: "nearest",
            behavior: "auto",
        });
    }, []);

    const onPageForward = useCallback((e: MouseEvent) => {
        e.preventDefault();
        setSearch((search) => ({ ...search, page: search.page + 1 }));

        postListTop.current?.scrollIntoView({
            block: "nearest",
            behavior: "auto",
        });
    }, []);

    useEffect(() => {
        if (search.page > maxPage) {
            setSearch((search) => ({ ...search, page: maxPage }));
        }
    }, [search.page, maxPage]);

    return (
        <div>
            <div
                className={"cdl-search-box-container" +
                    (condensed ? "is-condensed" : "")}
            >
                <SearchBox
                    search={search}
                    onChange={(changes) =>
                        setSearch((search) => ({ ...search, ...changes }))}
                    maxPage={maxPage}
                />
            </div>
            <div className="post-list">
                <div className="post-list-top" ref={postListTop} />

                {posts.map((post) => (
                    <div className="post-item my-4" key={post.postId}>
                        <PostPreview
                            condensed={condensed}
                            viewModel={post}
                            displayPrefs={displayPrefs}
                            highlightedTags={NO_TAGS}
                        />
                    </div>
                ))}

                {isLoading
                    ? (
                        <div className="cdl-loading-container">
                            <Loading />
                        </div>
                    )
                    : null}

                {errors.length
                    ? (
                        <div className="post-list-errors cohost-shadow-light rounded-lg bg-foreground p-4">
                            <h3 className="text-xl font-bold">
                                Error loading posts
                            </h3>
                            <ul>
                                {errors.map((error, i) => (
                                    <li key={i}>{error?.toString?.()}</li>
                                ))}
                            </ul>
                        </div>
                    )
                    : null}
            </div>
            <PaginationEggs
                condensed={condensed}
                backLink={search.page > 0 ? "#" : null}
                forwardLink={hasNextPage ? "#" : null}
                backOnClick={onPageBack}
                forwardOnClick={onPageForward}
            />
        </div>
    );
}

function ProjectIndex() {
    // UserInfoContext.Provider is in here because it will inexplicably not render anything if it's a parent of ProfileView
    return (
        <div className="flex flex-col">
            <div className="h-16 bg-foreground text-text">
                <div className="container mx-auto grid h-full items-center px-2">
                    <CohostLogo
                        className="h-8"
                        role="img"
                        aria-label="Cohost Archive"
                    />
                </div>
            </div>
            <div className="flex flex-grow flex-col pb-20">
                <LightboxHost>
                    <div className="container mx-auto flex flex-grow flex-col">
                        <ProfileView
                            project={project}
                            canAccessPermissions={ALL_VISIBLE}
                        >
                            <UserInfoContext.Provider value={GENERIC_OBSERVER}>
                                <FilteredPostFeed condensed />
                            </UserInfoContext.Provider>
                        </ProfileView>
                    </div>
                </LightboxHost>
            </div>
        </div>
    );
}

function AllPosts() {
    const [condensed, setCondensed] = useState(window.innerWidth < 1024);

    useEffect(() => {
        const updateCondensed = () => setCondensed(window.innerWidth < 1024);
        window.addEventListener("resize", updateCondensed);
        return () => {
            window.removeEventListener("resize", updateCondensed);
        };
    }, []);

    return (
        <div className="flex flex-col">
            <div className="h-16 bg-foreground text-text">
                <div className="container mx-auto grid h-full items-center px-2">
                    <CohostLogo
                        className="h-8"
                        role="img"
                        aria-label="Cohost Archive"
                    />
                </div>
            </div>
            <div className="flex flex-grow flex-col pb-20">
                <LightboxHost>
                    <main className="cdl-main">
                        <div className="cdl-all-projects">
                            <h3>Index</h3>
                            <ul>
                                {allProjects.map((project) => (
                                    <li>
                                        <a href={`../${project}/index.html`}>
                                            @{project}
                                        </a>
                                    </li>
                                ))}
                            </ul>
                        </div>
                        <div className="cdl-all-posts">
                            <UserInfoContext.Provider value={GENERIC_OBSERVER}>
                                <FilteredPostFeed condensed={condensed} />
                            </UserInfoContext.Provider>
                        </div>
                    </main>
                </LightboxHost>
            </div>
        </div>
    );
}

function App({ children }) {
    const initialI18nStore = JSON.parse(
        document.querySelector("#initialI18nStore")!.innerHTML,
    );
    const initialLanguage = JSON.parse(
        document.querySelector("#initialLanguage")!.innerHTML,
    );
    useSSR(initialI18nStore, initialLanguage);

    const [queryClient] = useState(() => new QueryClient());
    const [trpcClient] = useState(() =>
        trpc.createClient({
            links: [
                httpBatchLink({
                    url: sitemap.public.apiV1.trpc().toString(),
                    maxURLLength: 2083,
                    fetch(url, options) {
                        return new Promise(() => {
                        });
                    },
                }),
            ],
        })
    );

    return (
        <trpc.Provider client={trpcClient} queryClient={queryClient}>
            <QueryClientProvider client={queryClient}>
                <Hydrate state={trpcState}>
                    {children}
                </Hydrate>
            </QueryClientProvider>
        </trpc.Provider>
    );
}

const root = ReactDOM.createRoot(document.querySelector("#app"));

import("@/i18n").then(() => {
    root.render(
        <App>
            {project ? <ProjectIndex /> : null}
            {chunks ? <AllPosts /> : null}
        </App>,
    );
});
