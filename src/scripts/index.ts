import {
    clearDist,
    DIST_PATH,
    FrontendScript,
    generateFrontend,
} from "../script-compiler.ts";
import { CohostContext } from "../context.ts";

export const POST_PAGE_SCRIPT_PATH = `${DIST_PATH}/post-page.js`;
export const PROJECT_INDEX_SCRIPT_PATH = `${DIST_PATH}/post-index.js`;

export async function generateAllScripts(ctx: CohostContext, srcDir: string) {
    const POST_PAGE: FrontendScript = {
        name: "post-page",
        entryPoint: "post-page.tsx",
        base: "../..",
        additionalPatches: {
            "shared/sitemap.ts": [
                {
                    find: `if (path.startsWith("/srv/release/server/"))`,
                    replace: "return `../../static/${path}`; if (false)",
                }
            ],
        }
    };

    const POST_INDEX: FrontendScript = {
        name: "post-index",
        entryPoint: "post-index.tsx",
        base: "..",
        additionalPatches: {
            "preact/components/partials/post-tags.tsx": [
                {
                    find: "const isProfilePage = useMatch",
                    replace: `return {
                            pageType: window.cohostDL.project ? "profile" : false,
                            handle: window.cohostDL.project?.handle,
                            tagSlug: undefined,
                        };
                        const isProfilePage = useMatch`,
                },
            ],
            "preact/components/posts/post-footer.tsx": [
                {
                    find: "const numbersFlag =",
                    replace: "const numbersFlag = false; if (false)",
                },
                {
                    find: "const url = new URL(singlePostPageUrl);",
                    replace: "const url = new URL(singlePostPageUrl, location.href);",
                },
            ],
            "preact/components/posts/post-collapser.tsx": [
                {
                    find: "const currentUrl = useHref(useLocation());",
                    replace: "const currentUrl = '';",
                },
            ],
            "preact/components/token-input.tsx": [
                {
                    find: "...props",
                    replace: "onTagSearch = useTagSearch, ...props",
                },
                {
                    find: "= useTagSearch(",
                    replace: "= onTagSearch(",
                },
            ],
            "preact/providers/user-info-provider.tsx": [
                {
                    find: "const UserInfoContext",
                    replace: "export const UserInfoContext",
                },
            ],
            "shared/sitemap.ts": [
                {
                    find: `if (path.startsWith("/srv/release/server/"))`,
                    replace: "return `../static/${path}`; if (false)",
                },
                {
                    find: "mainDomain(patterns.public.project.mainAppProfile, args)",
                    replace: "`../${args.projectHandle}/index.html`"
                }
            ],
        },
    };

    await clearDist(ctx);
    await generateFrontend(ctx, srcDir, POST_INDEX);
    await generateFrontend(ctx, srcDir, POST_PAGE);
}
