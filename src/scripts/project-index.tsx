import React, { useMemo, useState } from "react";
import ReactDOM from "react-dom/client";
import {
    Hydrate,
    QueryClient,
    QueryClientProvider,
} from "@tanstack/react-query";
import sitemap from "@/shared/sitemap";
import { useSSR } from "react-i18next";
import { httpBatchLink } from "@trpc/client/links/httpBatchLink";
import { trpc } from "@/lib/trpc";
import { ProfileView } from "@/preact/components/partials/profile-view";
import { PostPreview } from "@/preact/components/posts/post-preview";
import { useDisplayPrefs } from "@/preact/hooks/use-display-prefs";
import { UserInfoContext } from "@/preact/providers/user-info-provider";
import { LightboxHost } from "@/preact/components/lightbox";
import { CohostLogo } from "@/preact/components/elements/icon";

const { project, posts, trpcState } = JSON.parse(
    document.querySelector("#project-index-data").innerHTML,
);
window.cohostDlProject = project;

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

function ProjectIndex() {
    const displayPrefs = useDisplayPrefs();

    const filteredPosts = useMemo(() => {
        return posts.sort((a, b) =>
            new Date(b.publishedAt) - new Date(a.publishedAt)
        );
    }, []);

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
                                {filteredPosts.map((post) => (
                                    <div className="my-4" key={post.postId}>
                                        <PostPreview
                                            condensed
                                            viewModel={post}
                                            displayPrefs={displayPrefs}
                                            highlightedTags={NO_TAGS}
                                        />
                                    </div>
                                ))}
                            </UserInfoContext.Provider>
                        </ProfileView>
                    </div>
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
                        return new Promise(() => {});
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
            <ProjectIndex />
        </App>,
    );
});
