import {
    clearDist,
    DIST_PATH,
    FrontendScript,
    generateFrontend,
} from "../script-compiler.ts";
import { CohostContext } from "../context.ts";

export const POST_PAGE_SCRIPT_PATH = `${DIST_PATH}/post-page.js`;
export const PROJECT_INDEX_SCRIPT_PATH = `${DIST_PATH}/project-index.js`;

export async function generateAllScripts(ctx: CohostContext, srcDir: string) {
    const POST_PAGE: FrontendScript = {
        name: "post-page",
        entryPointCode: `import "@/client.tsx"`,
        base: "../..",
    };

    const PROJECT_INDEX: FrontendScript = {
        name: "project-index",
        entryPointCode: await Deno.readTextFile(
            `${import.meta.dirname}/project-index.tsx`,
        ),
        base: "..",
        additionalPatches: {
            "preact/components/partials/post-tags.tsx": [
                {
                    find: "const isProfilePage = useMatch",
                    replace: `return {
                            pageType: "profile",
                            handle: window.cohostDlProject.projectHandle,
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
            "preact/providers/user-info-provider.tsx": [
                {
                    find: "const UserInfoContext",
                    replace: "export const UserInfoContext",
                },
            ],
        },
    };

    await clearDist(ctx);
    await generateFrontend(ctx, srcDir, PROJECT_INDEX);
    await generateFrontend(ctx, srcDir, POST_PAGE);
}