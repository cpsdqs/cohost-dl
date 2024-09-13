// this file is terrible. I'm sorry

import { rollup } from "npm:rollup";
import replace from "npm:@rollup/plugin-replace";
import commonjs from "npm:@rollup/plugin-commonjs";
import sucrase from "npm:@rollup/plugin-sucrase";
import * as path from "jsr:@std/path";
import { CohostContext } from "./context.ts";

import npmEntitiesDecode from "npm:entities/lib/maps/decode.json" with {
    type: "json",
};
import npmEntitiesEntities from "npm:entities/lib/maps/entities.json" with {
    type: "json",
};
import npmEntitiesLegacy from "npm:entities/lib/maps/legacy.json" with {
    type: "json",
};
import npmEntitiesXml from "npm:entities/lib/maps/xml.json" with {
    type: "json",
};
import npmEmojiMartData from "npm:@emoji-mart/data" with { type: "json" };

export const DIST_PATH = "~cohost-dl/dist";
export const POST_PAGE_SCRIPT_PATH = `${DIST_PATH}/post-page.js`;

const DEVELOPMENT = false;
const INCLUDE_DEV_PACKAGES = [
    "react",
    "react-dom",
    "react-is",
    "scheduler",
    "prop-types",
    "use-sync-external-store",
    "use-external-store-with-selector",
];

const PKG_ROOTS: Record<string, string> = {
    "@atlaskit/pragmatic-drag-and-drop": "dist/esm",
    "@atlaskit/pragmatic-drag-and-drop-hitbox": "dist/esm",
    "@emoji-mart/react": "dist/module.js",
    "@floating-ui/core": "dist/floating-ui.core.browser.min.mjs",
    "@floating-ui/dom": "dist/floating-ui.dom.browser.min.mjs",
    "@floating-ui/react-dom": "dist/floating-ui.react-dom.esm.js",
    "@headlessui/react": "dist",
    "@headlessui/tailwindcss": "dist",
    "@loadable/component": "dist/esm/loadable.esm.mjs",
    "@reduxjs/toolkit": "dist/redux-toolkit.modern.mjs",
    "@remix-run/router": "dist/router.js",
    "@selderee/plugin-htmlparser2": "lib/hp2-builder.cjs",
    "@tanstack/query-core": "build/lib",
    "@tanstack/react-query": "build/lib",
    "@trpc/client": "dist",
    "@trpc/react-query": "dist",
    "@trpc/server": "dist",
    "@unleash/proxy-client-react": "dist",
    "@xstate/react": "es",
    "bind-event-listener": "dist",
    "compute-scroll-into-view": "dist",
    "cross-fetch": "dist/browser-ponyfill.js",
    "cssesc": "cssesc.js",
    "decode-named-character-reference": "index.dom.js",
    "deepmerge": "dist/cjs.js",
    "dnd-core": "dist",
    "dom-serializer": "lib",
    "domelementttype": "lib",
    "domelementtype": "lib",
    "domhandler": "lib",
    "domutils": "lib",
    "downshift": "dist/downshift.esm.js",
    "emoji-mart": "dist/module",
    "goober": "dist/goober.modern.js",
    "hast-util-from-parse5": "lib",
    "hast-util-raw": "lib",
    "hast-util-to-html": "lib",
    "hast-util-to-parse5": "lib",
    "hastscript": "lib",
    "he": "he.js",
    "hoist-non-react-statics": "dist/hoist-non-react-statics.cjs.js",
    "html-parse-stringify": "dist/html-parse-stringify.module.js",
    "htmlparser2": "lib",
    "i18next": "dist/esm/i18next.js",
    "i18next-chained-backend": "dist/esm/i18nextChainedBackend.js",
    "i18next-http-backend": "esm/index.js",
    "i18next-localstorage-backend": "dist/esm/i18nextLocalStorageBackend.js",
    "immer": "dist/immer.mjs",
    "invariant": "browser.js",
    "lodash": "lodash.js",
    "luxon": "build/cjs-browser/luxon.js",
    "mdast-util-find-and-replace": "lib",
    "mdast-util-from-markdown": "lib",
    "mdast-util-gfm": "lib",
    "mdast-util-gfm-table": "lib",
    "mdast-util-newline-to-break": "lib",
    "mdast-util-to-hast": "lib",
    "micromark-core-commonmark": "lib",
    "micromark-extension-gfm-autolink-literal": "lib",
    "micromark-extension-gfm-footnote": "lib",
    "micromark-extension-gfm-strikethrough": "lib",
    "micromark-extension-gfm-table": "lib",
    "micromark-extension-gfm-task-list-item": "lib",
    "moo": "moo.js",
    "nearley": "lib/nearley.js",
    "parseley": "lib/parseley.cjs",
    "path-to-regexp": "dist.es2015",
    "picocolors": "picocolors.browser.js",
    "punycode": "punycode.js",
    "raf-schd": "dist/raf-schd.esm.js",
    "react-dnd": "dist",
    "react-dnd-html5-backend": "dist",
    "react-helmet-async": "lib/index.esm.js",
    "react-hook-form": "dist/index.esm.mjs",
    "react-hot-toast": "dist",
    "react-i18next": "dist/es",
    "react-masonry-css": "dist/react-masonry-css.module.js",
    "react-redux": "dist/react-redux.mjs",
    "react-render-if-visible": "dist",
    "react-router": "dist",
    "react-router-dom": "dist",
    "react-swipeable": "dist/react-swipeable.umd.js",
    "react-use": "esm",
    "redux": "dist/redux.mjs",
    "redux-thunk": "dist/redux-thunk.mjs",
    "rehype-react": "lib",
    "rehype-stringify": "lib",
    "remark-rehype": "lib",
    "reselect": "dist/reselect.mjs",
    "rollbar": "dist/rollbar.umd.min.js",
    "selderee": "lib/selderee.cjs",
    "stringify-entities": "lib",
    "tiny-invariant": "dist/esm/tiny-invariant.js",
    "tslib": "tslib.es6.js",
    "unified": "lib",
    "unist-util-is": "lib",
    "unist-util-visit-parents": "lib",
    "unleash-proxy-client": "build",
    "url": "url.js",
    "use-isomorphic-layout-effect":
        "dist/use-isomorphic-layout-effect.browser.esm.js",
    "uuid": "dist/esm-browser",
    "vfile": "lib",
    "xstate": "es",
    "zod": "lib",
    "@react-dnd/invariant": "dist",
    "@react-dnd/asap": "dist",
    "@react-dnd/shallowequal": "dist",
};

const IMPORT_VERSIONS: Record<
    string,
    { importer: string; version: string; restPrefix?: string }[]
> = {
    "unist-util-visit": [
        { importer: "@mapbox/hast-util-table-cell-style", version: "1.4.1" },
    ],
    "unist-util-visit-parents": [
        {
            importer: "unist-util-visit@1.4",
            version: "2.1.2",
            restPrefix: "../",
        },
    ],
};

const EXTRA_FILES: Record<string, string> = {
    "images/icons/favicons/apple-touch-icon.png":
        "https://cohost.org/static/7ec6f0f3aef87d734f9b.png",
    "images/icons/favicons/favicon-16x16.png":
        "https://cohost.org/static/3c154cde88b7ed1ca92a.png",
    "images/icons/favicons/favicon-32x32.png":
        "https://cohost.org/static/a4f72033a674e35d4cc9.png",
    "images/icons/favicons/favicon.ico":
        "https://cohost.org/static/1ba8b89b7a2ed8dd1d04.ico",
    "images/placeholders/attach.svg":
        "https://cohost.org/static/edcc39b1702e4bd4b95e.svg",
    "images/placeholders/attach_padding.svg":
        "https://cohost.org/static/edcc39b1702e4bd4b95e.svg",
    "images/anonbug.png": "https://cohost.org/static/ca4719f7af550ea00632.png",
    "images/thinkbug.png": "https://cohost.org/static/4fd0f5fb276c23f89e61.png",
};

const REWRITE_IDS: Record<string, string> = {
    // wonky roots
    "@/client/../tailwind.config.js": "@/tailwind.config.js",
    "../../shared/sitemap": "@/shared/sitemap",
    "../../../shared/sitemap": "@/shared/sitemap",
    "../../../shared/types/projects": "@/shared/types/projects",

    // older versions
    "@babel/runtime/helpers/esm/classCallCheck":
        "@babel/runtime@7.19.4/helpers/esm/classCallCheck",
    "@babel/runtime/helpers/esm/createClass":
        "@babel/runtime@7.19.4/helpers/esm/createClass",
    "@babel/runtime/helpers/defineProperty":
        "@babel/runtime@7.20.6/helpers/esm/defineProperty",
    "@babel/runtime/helpers/toConsumableArray":
        "@babel/runtime@7.20.6/helpers/esm/toConsumableArray",
    "@babel/runtime/helpers/extends":
        "@babel/runtime@7.20.6/helpers/esm/extends",
    "tailwindcss/lib/util/prefixNegativeModifiers":
        "tailwindcss@2.2.4_6wvqswfu3ceszl6jlfkyjqvkpi/lib/util/prefixNegativeModifiers",
    "tailwindcss/lib/util/flattenColorPalette":
        "tailwindcss@2.2.4_6wvqswfu3ceszl6jlfkyjqvkpi/lib/util/flattenColorPalette",
    "tailwindcss/lib/util/withAlphaVariable":
        "tailwindcss@2.2.4_6wvqswfu3ceszl6jlfkyjqvkpi/lib/util/withAlphaVariable",

    // etc
    "path": "path-browserify",
    "react-dom/server": "react-dom/server.browser",
    "@atlaskit/pragmatic-drag-and-drop/external/adapter":
        "@atlaskit/pragmatic-drag-and-drop/adapter/external-adapter",
    "@atlaskit/pragmatic-drag-and-drop/element/adapter":
        "@atlaskit/pragmatic-drag-and-drop/adapter/element-adapter",
    "@atlaskit/pragmatic-drag-and-drop/external/file":
        "@atlaskit/pragmatic-drag-and-drop/public-utils/external/file",
    "@atlaskit/pragmatic-drag-and-drop/reorder":
        "@atlaskit/pragmatic-drag-and-drop/public-utils/reorder",
    "@atlaskit/pragmatic-drag-and-drop/combine":
        "@atlaskit/pragmatic-drag-and-drop/public-utils/combine",
    "entities": "entities/lib/index.js",
    "use-sync-external-store/shim/with-selector":
        "use-sync-external-store/with-selector",

    // used for CSS only, as far as I can tell
    "../sass/main.scss": "@internal/nothing",
    "@tailwindcss/typography": "@internal/nothing",
    "@tailwindcss/forms": "@internal/nothing",
};

const SPECIALS: Record<string, string> = {
    "@emoji-mart/data": `export default ${JSON.stringify(npmEmojiMartData)}`,

    "@headlessui/react": [
        [
            "components/description/description.js",
            "Description",
            "useDescriptions",
        ],
        ["components/dialog/dialog.js", "Dialog"],
        ["components/disclosure/disclosure.js", "Disclosure"],
        ["components/focus-trap/focus-trap.js", "FocusTrap"],
        ["components/label/label.js", "Label", "useLabels"],
        ["components/listbox/listbox.js", "Listbox"],
        ["components/menu/menu.js", "Menu"],
        ["components/popover/popover.js", "Popover"],
        ["components/portal/portal.js", "Portal"],
        ["components/switch/switch.js", "Switch"],
        ["components/tabs/tabs.js", "Tab"],
        ["components/transitions/transition.js", "Transition"],
    ].map(([src, ...names]) =>
        `import { ${
            names.join(",")
        } } from "@headlessui/react/${src}"; export { ${names.join(",")} };`
    )
        .join("\n"),

    "@heroicons/react/20/solid": [
        "ArrowUpTrayIcon",
        "CheckCircleIcon",
        "ChevronDownIcon",
        "ChevronRightIcon",
        "ClockIcon",
        "CogIcon",
        "ExclamationTriangleIcon",
        "HashtagIcon",
        "QuestionMarkCircleIcon",
        "XCircleIcon",
    ].map((name) =>
        `import ${name} from "@heroicons/react/20/solid/esm/${name}.js"; export { ${name} };`
    ).join("\n"),

    "@heroicons/react/24/outline": [
        "ArrowLeftOnRectangleIcon",
        "LifebuoyIcon",
        "ArrowPathIcon",
        "LightBulbIcon",
        "ArrowRightOnRectangleIcon",
        "LockClosedIcon",
        "ArrowUpTrayIcon",
        "LockOpenIcon",
        "BellIcon",
        "MagnifyingGlassIcon",
        "ChatBubbleOvalLeftEllipsisIcon",
        "NewspaperIcon",
        "CheckCircleIcon",
        "NoSymbolIcon",
        "CheckIcon",
        "PaintBrushIcon",
        "ChevronDoubleLeftIcon",
        "PaperClipIcon",
        "ChevronDownIcon",
        "PauseCircleIcon",
        "ChevronLeftIcon",
        "PencilIcon",
        "ChevronRightIcon",
        "PencilSquareIcon",
        "CloudArrowDownIcon",
        "PlayCircleIcon",
        "CogIcon",
        "PlusIcon",
        "DocumentPlusIcon",
        "QuestionMarkCircleIcon",
        "DocumentTextIcon",
        "ShareIcon",
        "EllipsisHorizontalIcon",
        "ShieldExclamationIcon",
        "ExclamationCircleIcon",
        "SpeakerWaveIcon",
        "ExclamationTriangleIcon",
        "TagIcon",
        "EyeIcon",
        "TrashIcon",
        "EyeSlashIcon",
        "UserCircleIcon",
        "FaceSmileIcon",
        "UserGroupIcon",
        "HashtagIcon",
        "UserPlusIcon",
        "HeartIcon",
        "UsersIcon",
        "InboxIcon",
        "XMarkIcon",
        "InformationCircleIcon",
    ].map((name) =>
        `import ${name} from "@heroicons/react/24/outline/esm/${name}.js"; export { ${name} };`
    ).join("\n"),

    "@heroicons/react/24/solid": [
        "ArrowDownIcon",
        "ArrowPathIcon",
        "ArrowUpIcon",
        "ArrowUturnLeftIcon",
        "Bars3Icon",
        "BellIcon",
        "ChatBubbleOvalLeftEllipsisIcon",
        "CheckBadgeIcon",
        "CheckIcon",
        "ChevronDownIcon",
        "ChevronLeftIcon",
        "ChevronRightIcon",
        "ChevronUpDownIcon",
        "ChevronUpIcon",
        "CogIcon",
        "DocumentTextIcon",
        "EllipsisHorizontalIcon",
        "ExclamationCircleIcon",
        "HeartIcon",
        "InboxIcon",
        "LifebuoyIcon",
        "LinkIcon",
        "LockClosedIcon",
        "MagnifyingGlassIcon",
        "NewspaperIcon",
        "PaintBrushIcon",
        "UserCircleIcon",
        "UserGroupIcon",
        "UserIcon",
        "UserPlusIcon",
        "UsersIcon",
        "XMarkIcon",
    ].map((name) =>
        `import ${name} from "@heroicons/react/24/solid/esm/${name}.js"; export { ${name} };`
    ).join("\n") + `
export { HashtagIcon } from "@heroicons/react/20/solid";
    `,

    "@hookform/devtools": `export const DevTool = () => null;`,

    "@react-dnd/asap": `
export * from "@react-dnd/asap/asap";
export * from "@react-dnd/asap/AsapQueue";
export * from "@react-dnd/asap/makeRequestCall";
export * from "@react-dnd/asap/RawTask";
export * from "@react-dnd/asap/TaskFactory";
    `,

    "@tanstack/query-core": `
export * from "@tanstack/query-core/focusManager";
export * from "@tanstack/query-core/hydration";
export * from "@tanstack/query-core/infiniteQueryBehavior";
export * from "@tanstack/query-core/infiniteQueryObserver";
export * from "@tanstack/query-core/logger";
export * from "@tanstack/query-core/mutation";
export * from "@tanstack/query-core/mutationCache";
export * from "@tanstack/query-core/mutationObserver";
export * from "@tanstack/query-core/notifyManager";
export * from "@tanstack/query-core/onlineManager";
export * from "@tanstack/query-core/queriesObserver";
export * from "@tanstack/query-core/query";
export * from "@tanstack/query-core/queryCache";
export * from "@tanstack/query-core/queryClient";
export * from "@tanstack/query-core/queryObserver";
export * from "@tanstack/query-core/removable";
export * from "@tanstack/query-core/retryer";
export * from "@tanstack/query-core/subscribable";
export * from "@tanstack/query-core/utils";
    `,

    "@tanstack/react-query": `
export { QueryClient } from "@tanstack/query-core";
export * from "@tanstack/react-query/errorBoundaryUtils";
export * from "@tanstack/react-query/Hydrate";
export * from "@tanstack/react-query/isRestoring";
export * from "@tanstack/react-query/QueryClientProvider";
export * from "@tanstack/react-query/QueryErrorResetBoundary";
export * from "@tanstack/react-query/suspense";
export * from "@tanstack/react-query/useBaseQuery";
export * from "@tanstack/react-query/useInfiniteQuery";
export * from "@tanstack/react-query/useMutation";
export * from "@tanstack/react-query/useQueries";
export * from "@tanstack/react-query/useQuery";
export * from "@tanstack/react-query/useSyncExternalStore";
export * from "@tanstack/react-query/utils";

export function hashQueryKey() { throw new Error('not implemented') }
    `,

    "@trpc/client/links/httpBatchLink": `
import { h } from "@trpc/client/httpBatchLink-204206a5.mjs";
export { h as httpBatchLink };
    `,

    "@trpc/server/shared": `
import { T, a, b, c, g } from "@trpc/server/index-f91d720c.mjs";
export {
    T as TRPC_ERROR_CODES_BY_NUMBER,
    a as createRecursiveProxy,
    b as getHTTPStatusCode,
    c as createFlatProxy,
    g as getHTTPStatusCodeFromError,
};
    `,

    "@xstate/react": `
export { useActor } from "@xstate/react/useActor";
export { useConstant } from "@xstate/react/useConstant";
export { useInterpret } from "@xstate/react/useInterpret";
export { useMachine } from "@xstate/react/useMachine";
export { useSelector } from "@xstate/react/useSelector";
    `,

    "dnd-core": `
export { createDragDropManager } from "dnd-core/createDragDropManager";
    `,

    "hast-util-sanitize": `
export * from "hast-util-sanitize/lib";
export * from "hast-util-sanitize/lib/schema";
    `,

    "hastscript": `
import { h } from "hastscript/html";
import { s } from "hastscript/svg";
export { h, s };
    `,

    "micromark-core-commonmark": [
        "attention",
        "autolink",
        "blankLine",
        "blockQuote",
        "characterEscape",
        "characterReference",
        "codeFenced",
        "codeIndented",
        "codeText",
        "content",
        "definition",
        "hardBreakEscape",
        "headingAtx",
        "htmlFlow",
        "htmlText",
        "labelEnd",
        "labelStartImage",
        "labelStartLink",
        "lineEnding",
        "list",
        "setextUnderline",
        "thematicBreak",
    ].map((name) =>
        `export { ${name} } from "micromark-core-commonmark/${
            name.replace(/[A-Z]/g, (x) => "-" + x.toLowerCase())
        }.js"`
    ).join("\n"),

    "micromark-extension-gfm-autolink-literal": `
export * from "micromark-extension-gfm-autolink-literal/syntax";
export const gfmAutolinkLiteralHtml = null;
    `,
    "micromark-extension-gfm-footnote": `
export * from "micromark-extension-gfm-footnote/syntax";
export const gfmFootnoteHtml = null;
    `,
    "micromark-extension-gfm-strikethrough": `
export * from "micromark-extension-gfm-strikethrough/syntax";
export const gfmStrikethroughHtml = null;
    `,
    "micromark-extension-gfm-table": `
export * from "micromark-extension-gfm-table/syntax";
export const gfmTableHtml = null;
    `,
    "micromark-extension-gfm-tagfilter": `
export const gfmTagfilterHtml = null;
    `,
    "micromark-extension-gfm-task-list-item": `
export * from "micromark-extension-gfm-task-list-item/syntax";
export const gfmTaskListItemHtml = null;
    `,

    // missing, for some reason. let's assume it's just unused
    "mdast-util-gfm-autolink-literal": `
export function gfmAutolinkLiteralFromMarkdown() { return {} }
export function gfmAutolinkLiteralToMarkdown() { return {} }
    `,

    "react-dnd": `
export { useDrop } from "react-dnd/hooks/useDrop/useDrop";
export { DndProvider } from "react-dnd/core/DndProvider";
    `,

    "react-dnd/core": `
export * from "react-dnd/core/DndContext";
export * from "react-dnd/core/DndProvider";
    `,

    "react-dnd/internals": `
export * from "react-dnd/internals/DropTargetMonitorImpl";
export { isRef } from "react-dnd/internals/isRef";
export * from "react-dnd/internals/registration";
export { TargetConnector } from "react-dnd/internals/TargetConnector";
export * from "react-dnd/internals/wrapConnectorHooks";
    `,

    "react-i18next": `
export * from "react-i18next/context";
export * from "react-i18next/defaults";
export * from "react-i18next/i18nInstance";
export { initReactI18next } from "react-i18next/initReactI18next";
export { Trans } from "react-i18next/Trans";
export * from "react-i18next/TransWithoutContext";
export * from "react-i18next/unescape";
export * from "react-i18next/useSSR";
export * from "react-i18next/useTranslation";
export * from "react-i18next/utils";
    `,

    "react-use": `
export { default as useAsync } from "react-use/useAsync";
export { default as useAsyncFn } from "react-use/useAsyncFn";
export { default as useMedia } from "react-use/useMedia";
export { default as useMountedState } from "react-use/useMountedState";
    `,

    "unist-util-is/convert": `
import convert from "unist-util-is@3.0.0/../convert.js";
export default convert;
    `,

    "uuid": `
import NIL from "uuid/nil";
import v4 from "uuid/v4";
export { NIL, v4 };
    `,

    // bouba is missing. probably not important for anything
    "../icons/bouba": `
import { Kiki } from "@/preact/components/icons/kiki";
export { Kiki as Bouba }; // linguistics
    `,

    "entities/lib/maps/entities.json": `export default ${
        JSON.stringify(npmEntitiesEntities)
    }`,
    "entities/lib/maps/legacy.json": `export default ${
        JSON.stringify(npmEntitiesLegacy)
    }`,
    "entities/lib/maps/xml.json": `export default ${
        JSON.stringify(npmEntitiesXml)
    }`,
};

const naFn = (name: string) =>
    `export function ${name}() { throw new Error('not implemented') }`;

const MISSING_FILES: Record<string, { importer: string; contents: string }> = {
    "./getEmptyImage.js": {
        importer: "react-dnd-html5-backend",
        contents: naFn("getEmptyImage"),
    },
    "./match.js": { importer: "xstate", contents: naFn("matchState") },
    "./mapState.js": { importer: "xstate", contents: naFn("mapState") },
    "./schema.js": {
        importer: "xstate",
        contents: naFn("createSchema") + naFn("t"),
    },
    "./links/loggerLink.mjs": {
        importer: "@trpc/client",
        contents: naFn("loggerLink"),
    },
    "./util/format-basic.js": {
        importer: "stringify-entities",
        contents: naFn("formatBasic"),
    },

    "./adapters/http": { importer: "axios", contents: "export default null;" },
    "./headlessui.dev.cjs": {
        importer: "@headlessui/tailwindcss",
        contents: "export default null;",
    },
    "./cjs/react.development.js": {
        importer: "react",
        contents: "export default null;",
    },
    "./cjs/react-jsx-runtime.development.js": {
        importer: "react",
        contents: "export default null;",
    },
    "./cjs/react-dom.development.js": {
        importer: "react-dom",
        contents: "export default null;",
    },
    "./cjs/react-dom-server-legacy.browser.development.js": {
        importer: "react-dom",
        contents: "export default null;",
    },
    "./cjs/react-dom-server.browser.development.js": {
        importer: "react-dom",
        contents: "export default null;",
    },
    "./cjs/react-is.development.js": {
        importer: "react-is",
        contents: "export default null;",
    },
    "./cjs/scheduler.development.js": {
        importer: "scheduler",
        contents: "export default null;",
    },
    "./cjs/use-external-store-with-selector.development.js": {
        importer: "use-external-store-with-selector",
        contents: "export default null;",
    },
    "./cjs/use-sync-external-store-with-selector.development.js": {
        importer: "use-sync-external-store",
        contents: "export default null;",
    },
    "../cjs/use-sync-external-store-shim.development.js": {
        importer: "use-sync-external-store",
        contents: "export default null;",
    },
    "./useSyncExternalStoreShimServer.js": {
        importer: "@headlessui/react",
        contents: "export const useSyncExternalStore = null;",
    },
    "./factoryWithTypeCheckers": {
        importer: "prop-types",
        contents: "",
    },
    "./typedefs": { importer: "html-to-text", contents: "" },

    "./color.js": {
        importer: "unist-util-visit-parents",
        contents: "export function color(x) { return x }",
    },

    "./minpath.js": {
        importer: "vfile",
        contents: "export * from 'vfile/minpath.browser.js'",
    },
    "./minproc.js": {
        importer: "vfile",
        contents: "export * from 'vfile/minproc.browser.js'",
    },
    "./minurl.js": {
        importer: "vfile",
        contents: "export * from 'vfile/minurl.browser.js'",
    },

    "./maps/decode.json": {
        importer: "entities/lib",
        contents: `export default ${JSON.stringify(npmEntitiesDecode)}`,
    },
    "./maps/entities.json": {
        importer: "entities/lib",
        contents: `export default ${JSON.stringify(npmEntitiesEntities)}`,
    },
    "./maps/legacy.json": {
        importer: "entities/lib",
        contents: `export default ${JSON.stringify(npmEntitiesLegacy)}`,
    },
    "./maps/xml.json": {
        importer: "entities/lib",
        contents: `export default ${JSON.stringify(npmEntitiesXml)}`,
    },

    "../core/index.js": {
        importer: "react-dnd/dist/hooks/",
        contents: `export * from "react-dnd/core"`,
    },
    "../../internals/index.js": {
        importer: "react-dnd/dist/hooks/useDrop/",
        contents: `export * from "react-dnd/internals"`,
    },
};

const EMOJI = {
    "chunks.png": "https://cohost.org/static/f59b84127fa7b6c48b6c.png",
    "eggbug-classic.png": "https://cohost.org/static/41454e429d62b5cb7963.png",
    "eggbug.png": "https://cohost.org/static/17aa2d48956926005de9.png",
    "sixty.png": "https://cohost.org/static/9a6014af31fb1ca65a1f.png",
    "unyeah.png": "https://cohost.org/static/5cf84d596a2c422967de.png",
    "yeah.png": "https://cohost.org/static/014b0a8cc35206ef151d.png",
};
const PLUS_EMOJI = {
    "eggbug-asleep.png": "https://cohost.org/static/ebbf360236a95b62bdfc.png",
    "eggbug-devious.png": "https://cohost.org/static/c4f3f2c6b9ffb85934e7.png",
    "eggbug-heart-sob.png":
        "https://cohost.org/static/b59709333449a01e3e0a.png",
    "eggbug-nervous.png": "https://cohost.org/static/d2753b632211c395538e.png",
    "eggbug-pensive.png": "https://cohost.org/static/ae53a8b5de7c919100e6.png",
    "eggbug-pleading.png": "https://cohost.org/static/11c5493261064ffa82c0.png",
    "eggbug-relieved.png": "https://cohost.org/static/3633c116f0941d94d237.png",
    "eggbug-shocked.png": "https://cohost.org/static/b25a9fdf230219087003.png",
    "eggbug-smile-hearts.png":
        "https://cohost.org/static/d7ec7f057e6fb15a94cc.png",
    "eggbug-sob.png": "https://cohost.org/static/9559ff8058a895328d76.png",
    "eggbug-tuesday.png": "https://cohost.org/static/90058099e741e483208a.png",
    "eggbug-uwu.png": "https://cohost.org/static/228d3a13bd5f7796b434.png",
    "eggbug-wink.png": "https://cohost.org/static/3bc3a1c5272e2ceb8712.png",
    "host-aww.png": "https://cohost.org/static/9bb403f3822c6457baf6.png",
    "host-cry.png": "https://cohost.org/static/530f8cf75eac87716702.png",
    "host-evil.png": "https://cohost.org/static/cb9a5640d7ef7b361a1a.png",
    "host-frown.png": "https://cohost.org/static/99c7fbf98de865cc9726.png",
    "host-joy.png": "https://cohost.org/static/53635f5fe850274b1a7d.png",
    "host-love.png": "https://cohost.org/static/c45b6d8f9de20f725b98.png",
    "host-nervous.png": "https://cohost.org/static/e5d55348f39c65a20148.png",
    "host-plead.png": "https://cohost.org/static/fa883e2377fea8945237.png",
    "host-shock.png": "https://cohost.org/static/bfa6d6316fd95ae76803.png",
    "host-stare.png": "https://cohost.org/static/a09d966cd188c9ebaa4c.png",
};

function header(ctx: CohostContext) {
    const convertEmoji = (data: Record<string, string>) => {
        return Object.fromEntries(
            Object.entries(data).map((
                [k, v],
            ) => [k, ctx.propsForResourceURL(v)!.filePath]),
        );
    };

    return `if (!window.define) {
    const modules = {};
    window.__modules = modules;
    const modulePromises = {};

    const thisScriptSource = document.currentScript.getAttribute('src');
    const srcDir = thisScriptSource.substring(0, thisScriptSource.lastIndexOf('/'));

    const load = (id) => {
        if (modules[id]) return modules[id];
        if (modulePromises[id]) return modulePromises[id];
        return modulePromises[id] = new Promise((resolve, reject) => {
            const script = document.createElement('script');
            script.src = srcDir + '/' + id + '.js';
            script.onload = () => modules[id].then(resolve);
            script.onerror = err => reject(new Error('failed to load ' + id));
            script.dataset.id = id;
            document.head.append(script);
        });
    };

    const require = (ids, callback) => {
        Promise.all(ids.map(load)).then(items => {
            if (items.length === 1) callback(items[0]);
            else callback(items);
        });
    };
    
    require.context = (dir, useSubdirs) => {
        if ((dir === "../../images/emoji" || dir === "../images/emoji") && !useSubdirs) {
            const data = ${JSON.stringify(convertEmoji(EMOJI))};
            const f = (n) => data[n];
            f.keys = () => Object.keys(data);
            return f;
        } else if ((dir === "../../images/plus-emoji" || dir === "../images/plus-emoji") && !useSubdirs) {
            const data = ${JSON.stringify(convertEmoji(PLUS_EMOJI))};
            const f = (n) => data[n];
            f.keys = () => Object.keys(data);
            return f;
        }
        throw new Error('not supported: require.context for ' + dir);
    };

    window.define = (imports, exec) => {
        if (typeof imports === 'function') {
            create = imports;
            imports = [];
        }
        const id = document.currentScript.dataset.id
            ?? './' + document.currentScript.getAttribute('src').split('/').pop().replace(/\\.js$/i, '');
        if (modules[id]) return;
        const exports = {};
        modules[id] = Promise.resolve().then(function() {
            const imported = [];
            for (const id of imports) {
                if (id === 'require') imported.push(Promise.resolve(require));
                else if (id === 'exports') imported.push(Promise.resolve(exports));
                else imported.push(load(id));
            }
            return Promise.all(imported);
        }).then(function(imported) {
            const result = exec.apply(window, imported);
            if (!('default' in exports)) exports.default = result;
            return exports;
        });
    };

    window.process = { env: { NODE_ENV: '${
        DEVELOPMENT ? "development" : "production"
    }' } };
}
`;
}

interface PatchReplace {
    find: string;
    replace: string;
    multi?: boolean;
}
type Patch = PatchReplace;

const REFETCH_NONE =
    "refetchInterval: false, refetchOnReconnect: false, refetchOnWindowFocus: false, refetchIntervalInBackground: false,";

const IMPL_REWRITE_CDN_URLS = `
import { generate as cssGenerate, parse as cssParse, walk as cssWalk } from "@internal/css-tree";
function rewriteCdnUrls() {
    const rewriteDataNode = document.getElementById('__cohost_dl_rewrite_data');
    const rewriteData = rewriteDataNode ? JSON.parse(rewriteDataNode.innerHTML) : null;

    function rewriteUrl(url) {
        if (rewriteData?.urls?.[url]) return rewriteData.urls[url];
        return null;
    }

    return function(tree) {
        const process = (node) => {
            if (node.properties?.style) {
                let mutated = false;
                const tree = cssParse(node.properties.style, { context: 'declarationList' });

                const nodes: { value: string }[] = [];
                cssWalk(tree, (node: { type: string; value: string }) => {
                    if (node.type === "Url") {
                        const resolved = new URL(node.value, rewriteData.base);
                        const rewritten = rewriteUrl(resolved.toString());
                        if (rewritten) {
                            mutated = true;
                            node.value = rewritten;
                        }
                    }
                });

                if (mutated) {
                    node = { ...node, properties: { ...node.properties, style: cssGenerate(tree) } };
                }
            }

            const props = ['href', 'src'];
            for (const prop of props) {
                if (node.properties?.[prop]) {
                    const resolved = new URL(node.properties[prop], rewriteData.base);
                    const rewritten = rewriteUrl(resolved.toString());
                    if (rewritten) {
                        node = { ...node, properties: { ...node.properties, [prop]: rewritten } };
                    }
                }
            }
            if (node.properties?.srcset) {
                node = { ...node, properties: { ...node.properties } };
                delete node.properties.srcset;
            };

            if (node.children) {
                let mutated = false;
                const newChildren = node.children.map(child => {
                    const result = process(child);
                    mutated = result !== child;
                    return result;
                });
                if (mutated) return { ...node, children: newChildren };
            }
            return node;
        };
        return process(tree);
    };
}
`;

const PATCHES: Record<string, Patch[]> = {
    "i18n.ts": [
        {
            find: "loadPath: `/rc/locales/",
            replace: "loadPath: `../../rc/locales/",
        },
    ],
    "client.tsx": [
        {
            find: "void loadableReady(setupApp);",
            replace: "void setupApp();",
        },
        {
            find: "const AsyncPage =",
            replace:
                `import singlePostViewPage from '@/client/preact/components/pages/single-post-view';
                const AsyncPage = singlePostViewPage;
                const OLD_AsyncPage =`,
        },
        {
            find: "import(`@/client/preact/components/pages/${props.page}`)",
            replace: (() => {
                const syncPages: Record<string, string> = {
                    "single-post-view": "singlePostViewPage",
                };

                let out = "(() => {\n";
                for (const [page, varName] of Object.entries(syncPages)) {
                    out += `if (props.page === ${JSON.stringify(page)}) {`;
                    out += `return Promise.resolve(${varName});`;
                    out += `}\n`;
                }
                out += "throw new Error('unknown page: ' + props.page)\n";
                out += "})()";
                return out;
            })(),
        },
    ],
    "layouts/main.tsx": [
        {
            find: "<Suspense>{children}</Suspense>",
            replace: "{children}",
        },
    ],
    "shared/env.ts": [
        {
            find: "get HOME_URL() {",
            replace:
                "get HOME_URL() { return new URL('../..', location.href).toString() } _doNothing() {",
        },
    ],
    "preact/components/posts/blocks/attachments/audio.tsx": [
        {
            find: "pathEntries = new URL(block.attachment.fileURL)",
            replace:
                "pathEntries = new URL(block.attachment.fileURL, location.href)",
        },
    ],
    "preact/components/partials/project-avatar.tsx": [
        {
            find: "parsedSrc = new URL(src)",
            replace: "parsedSrc = new URL(src, location.href); return src",
        },
    ],
    "preact/hooks/data-loaders.ts": [
        {
            find: "refetchInterval: 1000 * 30,",
            replace: REFETCH_NONE,
            multi: true,
        },
    ],
    "preact/hooks/use-image-optimizer.ts": [
        {
            find: "const parsedSrc = new URL(src);",
            replace: "const parsedSrc = new URL(src, document.location.href);",
        },
    ],
    "preact/providers/user-info-provider.tsx": [
        {
            find: "refetchInterval: 15 * 1000,",
            replace: REFETCH_NONE,
            multi: true,
        },
    ],
    "node_modules/.pnpm/@trpc+client@10.40.0_@trpc+server@10.40.0/node_modules/@trpc/client/dist/httpUtils-f58ceda1.mjs":
        [
            {
                find: "async function fetchHTTPResponse(opts, ac) {",
                // never resolve. just stop making requests please
                replace:
                    "async function fetchHTTPResponse(opts, ac) { return new Promise(() => {});",
            },
        ],
    "lib/markdown/post-rendering.tsx": [
        {
            find: "const markdownRenderStack =",
            replace: `${IMPL_REWRITE_CDN_URLS}\nconst markdownRenderStack =`,
        },
        {
            find: ".use(rehypeExternalLinks, {",
            replace: ".use(rewriteCdnUrls).use(rehypeExternalLinks, {",
        },
    ],
    "lib/markdown/other-rendering.ts": [
        {
            find: "const markdownRenderStackNoHTML =",
            replace:
                `${IMPL_REWRITE_CDN_URLS}\nconst markdownRenderStackNoHTML =`,
        },
        {
            find: ".use(rehypeSanitize, effectiveSchema)",
            replace:
                ".use(rehypeSanitize, effectiveSchema).use(rewriteCdnUrls)",
        },
    ],

    // PATCHES FOR HYDRATION REASONS
    "preact/components/partials/topnav.tsx": [{
        find: "sitemap.public.home()",
        replace: "'https://cohost.org/'",
    }],
};

const POST_PAGE = `
import "@/client.tsx";
`;

export async function generatePostPageScript(
    ctx: CohostContext,
    srcDir: string,
) {
    console.log("compiling post page script");

    const cssTreeSourcePath = await ctx.loadResourceToFile(
        "https://esm.sh/v135/css-tree@2.3.1/es2022/css-tree.bundle.mjs",
    );
    if (!cssTreeSourcePath) {
        throw new Error("could not load css-tree");
    }
    const cssTreeSource = await ctx.readText(cssTreeSourcePath);

    const extraFiles = Object.fromEntries(
        Object.entries(EXTRA_FILES).map((
            [k, v],
        ) => [ctx.getCleanPath(`${srcDir}/${k}`), v]),
    );

    for (
        const file of [...Object.values(EMOJI), ...Object.values(PLUS_EMOJI)]
    ) {
        await ctx.loadResourceToFile(file);
    }

    const realSrcDir = await Deno.realPath(ctx.getCleanPath(srcDir));

    try {
        await Deno.remove(ctx.getCleanPath(DIST_PATH), { recursive: true });
    } catch {
        // not important
    }

    const npmPackages: Record<string, string> = {};

    const npmPackageItems: string[] = [];
    for await (
        const item of Deno.readDir(
            ctx.getCleanPath(`${srcDir}/node_modules/.pnpm`),
        )
    ) {
        npmPackageItems.push(item.name);
    }
    npmPackageItems.sort();

    const npmPackageVersions: Record<string, string> = {
        "@emoji-mart/data": "1.2.1",
    };

    for (const item of npmPackageItems) {
        const versionIndex = item.indexOf("@", 1);
        const pkgName = item.substring(0, versionIndex).replace(
            /[+]/g,
            "/",
        );

        let pkgRoot =
            `${srcDir}/node_modules/.pnpm/${item}/node_modules/${pkgName}`;

        if (PKG_ROOTS[pkgName]) pkgRoot += `/${PKG_ROOTS[pkgName]}`;

        npmPackageVersions[pkgName] =
            item.substring(versionIndex + 1).split("_")[0];

        npmPackages[item.substring(0, versionIndex)] = pkgRoot;
        npmPackages[item] = pkgRoot;
    }

    const entryName = "post-page";

    const bundle = await rollup({
        input: `@internal/${entryName}`,
        plugins: [
            {
                name: "cohost-dl-resolve",
                load(id) {
                    if (id === "@internal/nothing") return "";

                    if (id === `@internal/${entryName}`) return POST_PAGE;

                    if (id === "@internal/css-tree") {
                        return cssTreeSource;
                    }

                    if (id.startsWith("@internal/missing ")) {
                        const missingId = id.split(" ")[1];
                        return MISSING_FILES[missingId]?.contents ?? "";
                    }

                    if (id.startsWith("out/static/")) {
                        return `export default ${
                            JSON.stringify(path.basename(id))
                        }`;
                    }

                    if (id.startsWith("@internal/special")) {
                        const specialId = id.split("{")[1].split("}")[0];
                        return SPECIALS[specialId];
                    }

                    if (
                        DEVELOPMENT &&
                        (id.startsWith("https://") || id.startsWith("data:"))
                    ) {
                        const filePath = `~dev-cache/${encodeURIComponent(id)}`;
                        return ctx.readText(filePath).catch(async () => {
                            const res = await fetch(id.slice(0, -3), {
                                headers: {
                                    // we don't want deno packages
                                    "User-Agent":
                                        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:130.0) Gecko/20100101 Firefox/130.0",
                                },
                            });
                            if (!res.ok) throw new Error(await res.text());
                            const text = await res.text();
                            await ctx.write(filePath, text);
                            return text;
                        });
                    }

                    return null;
                },
                async resolveId(originalId, importer) {
                    if (originalId.includes("Trans.js")) {
                        console.log(originalId, importer);
                    }

                    let resolved: string | null = null;

                    let id = originalId;
                    if (SPECIALS[id]) {
                        return `@internal/special{${id}}.js`;
                    }

                    if (REWRITE_IDS[id]) id = REWRITE_IDS[id];

                    if (DEVELOPMENT && importer?.startsWith("https://")) {
                        // dist/es -> dist/es/
                        const importer2 = importer.slice(0, -3).match(
                                /@([\d.]+)((\/.*)?\/esm?|\/dist|\/shim)?$/,
                            )
                            ? importer.slice(0, -3) + "/"
                            : importer.slice(0, -3);
                        if (originalId.includes("shim.development")) {
                            console.log({
                                originalId,
                                importer,
                                importer2,
                                x: new URL(originalId, importer2).toString(),
                            });
                        }
                        if (originalId.match(/^[^a-zA-Z@]/)) {
                            return new URL(originalId, importer2).toString() +
                                ".js";
                        }
                    }

                    if (id.startsWith("@fontsource/")) {
                        return "@internal/nothing";
                    } else if (id.startsWith("@internal/")) {
                        return id;
                    } else if (id.startsWith("@/client/")) {
                        resolved = srcDir + id.substring("@/client".length);
                    } else if (id.startsWith("@/")) {
                        resolved = srcDir + id.substring(1);
                    } else if (id.startsWith("@")) {
                        const [name, pkg, ...rest] = id.split("/");

                        if (
                            DEVELOPMENT &&
                            INCLUDE_DEV_PACKAGES.includes(`${name}/${pkg}`)
                        ) {
                            const pkgName = `${name}/${pkg}`;
                            return `https://unpkg.com/${name}/${pkg}${
                                npmPackageVersions[pkgName]
                                    ? "@" + npmPackageVersions[pkgName]
                                    : ""
                            }${
                                PKG_ROOTS[pkgName]
                                    ? `/${PKG_ROOTS[pkgName]}`
                                    : ""
                            }${rest.length ? `/${rest.join("/")}` : ""}.js`;
                        }

                        const pkgName = `${name}+${pkg}`;

                        if (npmPackages[pkgName]) {
                            resolved = `${npmPackages[pkgName]}${
                                rest.length ? `/${rest.join("/")}` : ""
                            }`;
                        }
                    } else if (!id.startsWith(".")) {
                        const [originalPkg, ...restItems] = id.split("/");

                        let pkg = originalPkg;
                        let rest = restItems.join("/");

                        if (
                            DEVELOPMENT &&
                            INCLUDE_DEV_PACKAGES.includes(originalPkg)
                        ) {
                            // need to add .js for commonjs to take effect
                            return `https://unpkg.com/${pkg}${
                                npmPackageVersions[pkg]
                                    ? "@" + npmPackageVersions[pkg]
                                    : ""
                            }${PKG_ROOTS[pkg] ? `/${PKG_ROOTS[pkg]}` : ""}${
                                rest ? `/${rest}` : ""
                            }.js`;
                        }

                        for (const item of IMPORT_VERSIONS[pkg] ?? []) {
                            if (importer?.includes(item.importer)) {
                                pkg += "@" + item.version;
                                if (item.restPrefix) {
                                    rest = item.restPrefix + rest;
                                }
                                break;
                            }
                        }

                        if (npmPackages[pkg]) {
                            resolved = `${npmPackages[pkg]}${
                                rest ? `/${rest}` : ""
                            }`;
                        }
                    }

                    if (resolved) {
                        const resolvedPath = ctx.getCleanPath(resolved);
                        const resolvedDir = path.dirname(resolvedPath);
                        const resolvedFileName = path.basename(resolvedPath);

                        if (extraFiles[resolvedPath]) {
                            const loaded = await ctx.loadResourceToFile(
                                extraFiles[resolvedPath],
                            );
                            return ctx.getCleanPath(loaded as string);
                        }

                        let stat: Deno.FileInfo | null = null;
                        try {
                            stat = await Deno.stat(resolvedPath);
                        } catch { /* nothing */ }

                        if (stat?.isFile) {
                            return await Deno.realPath(resolvedPath);
                        } else if (stat?.isDirectory) {
                            for await (
                                const item of Deno.readDir(resolvedPath)
                            ) {
                                if (
                                    item.name.match(
                                        /^index[.]((esm|browser)[.])?([cm]?js|jsx|ts|tsx)$/,
                                    )
                                ) {
                                    return await Deno.realPath(
                                        `${resolvedPath}/${item.name}`,
                                    );
                                }
                            }
                        } else {
                            try {
                                for await (
                                    const item of Deno.readDir(resolvedDir)
                                ) {
                                    if (!item.isFile) continue;

                                    const extIndex = item.name.lastIndexOf(".");
                                    if (extIndex > -1) {
                                        const name = item.name.substring(
                                            0,
                                            extIndex,
                                        );
                                        if (name === resolvedFileName) {
                                            return await Deno.realPath(
                                                `${resolvedDir}/${item.name}`,
                                            );
                                        }
                                    }
                                }
                            } catch (err) {
                                throw new Error(
                                    `Error resolving ${id} from ${importer}: ${err}`,
                                );
                            }
                        }

                        throw new Error(
                            `cannot resolve ${originalId}: trying ${resolved} (importer: ${importer})`,
                        );
                    }

                    if (
                        MISSING_FILES[id] &&
                        importer?.includes(MISSING_FILES[id].importer)
                    ) {
                        return `@internal/missing ${id}`;
                    }

                    return null;
                },
            },
            {
                name: "cohost-dl-transform",
                transform(code, originalId) {
                    const id = path.relative(realSrcDir, originalId);

                    if (PATCHES[id]) {
                        for (const patch of PATCHES[id]) {
                            if ("find" in patch) {
                                for (let i = 0; i < 256; i++) {
                                    const index = code.indexOf(patch.find);
                                    if (index === -1) {
                                        if (i === 0) {
                                            throw new Error(
                                                `could not patch ${id}: missing "${patch.find}"`,
                                            );
                                        } else {
                                            break;
                                        }
                                    }
                                    code = code.substring(0, index) +
                                        patch.replace +
                                        code.substring(
                                            index + patch.find.length,
                                        );

                                    if (!patch.multi) break;
                                }
                            }
                        }
                        console.log(`patched ${id}`);
                        return code;
                    }

                    return null;
                },
            },
            replace({
                "process.env.NODE_ENV": DEVELOPMENT
                    ? "'development'"
                    : "'production'",
                preventAssignment: true,
            }),
            commonjs(),
            sucrase({
                exclude: ["node_modules/**"],
                transforms: ["jsx", "typescript"],
                jsxRuntime: "automatic",
                production: true,
                disableESTransforms: true,
            }),
        ],
        context: "this",
        onwarn(m, log) {
            if (
                m.code === "MODULE_LEVEL_DIRECTIVE" &&
                m.message.includes('"use client"')
            ) {
                return;
            }
            if (m.code === "UNRESOLVED_IMPORT") {
                log(m);
                throw new Error("unresolved!");
            }
            if (
                m.code === "CIRCULAR_DEPENDENCY" &&
                m.message.includes("axios@0.24.0")
            ) {
                return;
            }
            log(m);
        },
    });

    const { output } = await bundle.generate({
        banner: header(ctx),
        dir: "",
        format: "amd",
    });

    for (const item of output) {
        const path = `${DIST_PATH}/${item.fileName}`;
        if (item.type === "asset") {
            await ctx.write(path, item.source);
        } else if (item.type === "chunk") {
            await ctx.write(path, item.code);
        }
    }
}
