import * as path from "jsr:@std/path";
import { Document, DOMParser } from "jsr:@b-fuze/deno-dom";
import {
    generate as cssGenerate,
    parse as cssParse,
    walk as cssWalk,
} from "npm:css-tree@2.3.1";
import { DO_NOT_FETCH_HOSTNAMES } from "./config.ts";

const USER_AGENT = "cohost-dl/1.0";
const MAX_FILE_NAME_LENGTH_UTF8 = 250;

export function splitTooLongFileName(filePath: string): string {
    let dirname = path.dirname(filePath);
    let filename = path.basename(filePath);

    while (
        new TextEncoder().encode(filename).byteLength >
            MAX_FILE_NAME_LENGTH_UTF8
    ) {
        let firstBit = "";
        let rest = filename;
        while (
            new TextEncoder().encode(firstBit).byteLength <
                MAX_FILE_NAME_LENGTH_UTF8
        ) {
            const item = rest[Symbol.iterator]().next().value;
            firstBit += item;
            rest = rest.substring(item.length);
        }

        dirname = path.join(dirname, firstBit);
        filename = rest;
    }

    return path.join(dirname, filename);
}

const NORMAL_FILE_EXTENSIONS = {
    // image formats
    "apng": "image/apng",
    "avif": "image/avif",
    "bmp": "image/bmp",
    "gif": "image/gif",
    "heic": "image/heic",
    "heif": "image/heif",
    "ico": "image/x-icon",
    "jpeg": "image/jpeg",
    "jpg": "image/jpeg",
    "jfif": "image/jpeg",
    "jxl": "image/jxl",
    "png": "image/png",
    "svg": ["image/svg+xml", "image/svg"],
    "tif": "image/tiff",
    "tiff": "image/tiff",
    "webp": "image/webp",

    // av formats
    "flac": "audio/flac",
    "ogg": ["audio/ogg", "video/ogg", "application/ogg"],
    "opus": "audio/opus",
    "mp3": "audio/mpeg",
    "mp4": ["audio/mp4", "video/mp4"],
    "m4a": ["audio/mp4", "video/mp4"],
    "wav": ["audio/wav", "audio/vnd.wave", "audio/wave", "audio/x-wav"],

    // other resources
    "css": "text/css",
    "js": ["application/javascript", "text/javascript"],
    "mjs": ["application/javascript", "text/javascript"],
    "json": ["application/json", "text/json"],
    "map": [] as string[],
    "woff": ["font/woff"],
    "woff2": ["font/woff2"],
};

function doesResourceFilePathProbablyNeedAFileExtension(
    filePath: string,
): boolean {
    const fileExtension = path.extname(filePath).toLowerCase().replace(
        /^[.]/,
        "",
    );
    return !(fileExtension in NORMAL_FILE_EXTENSIONS);
}

function resourceFileExtensionForContentType(
    contentType: string,
): string | null {
    const baseContentType = contentType.split(";")[0];

    for (const [ext, types] of Object.entries(NORMAL_FILE_EXTENSIONS)) {
        if (
            (Array.isArray(types) && types.includes(baseContentType)) ||
            types === baseContentType
        ) return ext;
    }
    return null;
}

export const POST_URL_REGEX = /^https:\/\/cohost[.]org\/([^\/]+)\/post\/(\d+)-/;

const CACHED_CONTENT_TYPES_FILE_PATH = "~headers.json";

export function encodeFilePathURI(path: string): string {
    // encodeURI preserves ?search, which we don’t want
    return path.split('/').map(encodeURIComponent).join('/');
}

export class CohostContext {
    /** cookie header */
    cookie: string;

    /** output directory for files */
    rootDir: string;

    cachedContentTypes: Record<string, Record<string, string>> = {};

    constructor(cookie: string, rootDir: string) {
        this.cookie = cookie;
        this.rootDir = rootDir;
    }

    async init() {
        try {
            this.cachedContentTypes = (await this.readJson(
                CACHED_CONTENT_TYPES_FILE_PATH,
            )) as typeof this.cachedContentTypes;
        } catch {
            // not important
        }
    }

    async finalize() {
        await this.flushCachedContentTypes();
    }

    getCachedContentType(url: string): string | null {
        return this.cachedContentTypes[url]?.["content-type"] ?? null;
    }

    setCachedContentType(url: string, contentType: string) {
        this.cachedContentTypes[url] = { "content-type": contentType };
        this.scheduleFlushCachedContentTypes();
    }

    scheduledFlushContentTypes: ReturnType<typeof setTimeout> | null = null;
    scheduleFlushCachedContentTypes() {
        if (this.scheduledFlushContentTypes) return;
        this.scheduledFlushContentTypes = setTimeout(() => {
            const _ = this.flushCachedContentTypes();
        }, 5000);
    }

    async flushCachedContentTypes() {
        if (this.scheduledFlushContentTypes) {
            clearTimeout(this.scheduledFlushContentTypes);
        }
        this.scheduledFlushContentTypes = null;

        try {
            await this.write(
                CACHED_CONTENT_TYPES_FILE_PATH,
                JSON.stringify(this.cachedContentTypes),
            );
        } catch (error) {
            console.error("failed to write content types cache!");
            console.error(error);
        }
    }

    /** Performs a GET request */
    async get(url: string): Promise<Response> {
        console.log(`GET ${url}`);
        const res = await fetch(url, {
            headers: {
                "user-agent": USER_AGENT,
                ...(new URL(url).host === "cohost.org"
                    ? { cookie: this.cookie }
                    : {}),
            },
        });

        if (!res.ok) {
            throw new Error(await res.text());
        }

        return res;
    }

    /** Beware when using this method with external files, because it may not return the final filePath */
    propsForResourceURL(
        urlString: string,
    ): {
        fetch: string;
        filePath: string;
        canFail?: boolean;
        skipFileExtCheck?: boolean;
    } | null {
        const url = new URL(urlString);
        if (
            url.hostname === "staging.cohostcdn.org" &&
            url.pathname.match(/^\/[a-z]+\//)
        ) {
            return {
                fetch: `https://staging.cohostcdn.org${url.pathname}`,
                filePath: `rc${decodeURIComponent(url.pathname)}`,
                skipFileExtCheck: true,
            };
        } else if (url.hostname === "cohost.org") {
            return {
                fetch: urlString,
                filePath: decodeURIComponent(url.pathname.substring(1)), // no leading /
                skipFileExtCheck: true,
            };
        } else if (
            url.protocol === "https:" &&
            !DO_NOT_FETCH_HOSTNAMES.includes(url.hostname)
        ) {
            return {
                fetch: urlString,
                filePath: splitTooLongFileName(
                    `rc/external/${url.host}${url.pathname}${url.search}`,
                ),
                canFail: true,
            };
        } else {
            console.error(`IGNORING URL ${urlString}`);
        }

        return null;
    }

    pendingResources = new Map<string, Promise<string | null>>();

    /** Ensure a valid path on disk relative to the root directory */
    getCleanPath(filePath: string): string {
        if (Deno.build.os === "windows") {
            // replace illegal characters for windows paths
            const cleanFilePath = filePath.replace(/[?%*:|"<>]/g, "-");
            return path.join(this.rootDir, cleanFilePath);
        }

        return path.join(this.rootDir, filePath);
    }

    async hasFile(filePath: string): Promise<boolean> {
        const fullPath = this.getCleanPath(filePath);
        try {
            await Deno.stat(fullPath);
            return true;
        } catch {
            return false;
        }
    }

    getCachedFileForPostURL(url: string): Promise<string | null> {
        const match = url.match(POST_URL_REGEX);
        if (!match) return Promise.resolve(null);
        const [, projectHandle, id] = match;

        return this.getCachedFileForPost(projectHandle, id);
    }

    async getCachedFileForPost(projectHandle: string, id: string | number): Promise<string | null> {
        const projectDir = path.join(this.getCleanPath(projectHandle), "post");
        try {
            for await (const item of Deno.readDir(projectDir)) {
                if (
                    item.name.startsWith(id + "-") &&
                    item.name.endsWith(".html")
                ) {
                    return `${projectHandle}/post/${item.name}`;
                }
            }
        } catch {
            // readDir failed - probably because the directory doesn't exist
        }

        return null;
    }

    readText(filePath: string): Promise<string> {
        const fullPath = this.getCleanPath(filePath);
        return Deno.readTextFile(fullPath);
    }

    async readJson(filePath: string): Promise<object> {
        return JSON.parse(await this.readText(filePath));
    }

    async write(filePath: string, data: string | Uint8Array) {
        const fullPath = this.getCleanPath(filePath);

        await Deno.mkdir(path.dirname(fullPath), { recursive: true });
        await Deno.writeFile(
            fullPath,
            typeof data === "string" ? new TextEncoder().encode(data) : data,
        );
    }

    async loadResourceToFile(urlString: string): Promise<string | null> {
        const props = this.propsForResourceURL(urlString);
        if (!props) return null;

        const needsFileExtension = !props.skipFileExtCheck &&
            doesResourceFilePathProbablyNeedAFileExtension(props.filePath);

        let forceFailStat = false;
        let filePathWithExt = props.filePath;

        if (needsFileExtension) {
            const cachedContentType = this.getCachedContentType(props.fetch);
            if (cachedContentType === null) {
                // from an older version of this script & probably broken. we need to reload this one
                forceFailStat = true;
            } else {
                const ext = resourceFileExtensionForContentType(
                    cachedContentType,
                );
                if (ext !== null) filePathWithExt = props.filePath + "." + ext;
            }
        }

        const fullPath = this.getCleanPath(filePathWithExt);

        try {
            if (forceFailStat) throw new Error("missing extension");

            await Deno.stat(fullPath);
            return filePathWithExt;
        } catch {
            const pending = this.pendingResources.get(urlString);
            if (pending) return pending;

            const pending2 = (async () => {
                let res: Response;
                try {
                    res = await this.get(props.fetch);
                } catch (err) {
                    if (props.canFail) {
                        console.error(`FAILED: GET ${props.fetch}`);
                        return null;
                    } else {
                        throw err;
                    }
                }

                const contentTypeHeader = res.headers.get("content-type");
                if (contentTypeHeader) {
                    this.setCachedContentType(props.fetch, contentTypeHeader);
                }

                if (needsFileExtension && contentTypeHeader) {
                    const ext = resourceFileExtensionForContentType(
                        contentTypeHeader,
                    );
                    if (ext !== null) {
                        filePathWithExt = props.filePath + "." + ext;
                    }
                }

                const baseContentType = contentTypeHeader?.split(";")?.[0];

                let data: string | Uint8Array;
                if (baseContentType === "text/css") {
                    data = await this.processCss(
                        urlString,
                        props.filePath,
                        await res.text(),
                    );
                } else {
                    data = new Uint8Array(await res.arrayBuffer());
                }

                await this.write(filePathWithExt, data);

                return filePathWithExt;
            })();
            this.pendingResources.set(urlString, pending2);
            return pending2;
        }
    }

    async processCss(
        urlString: string,
        filePath: string,
        contents: string,
    ): Promise<string> {
        const tree = cssParse(contents);
        const nodes: { value: string }[] = [];
        cssWalk(tree, (node: { type: string; value: string }) => {
            if (node.type === "Url") {
                nodes.push(node);
            }
        });

        const toRootDir =
            path.dirname(filePath).split("/").map(() => "..").join("/") + "/";

        await Promise.all(nodes.map(async (node) => {
            const resolved = new URL(node.value, urlString);
            if (resolved.protocol !== "https:") return;

            const filePath = await this.loadResourceToFile(resolved.toString());
            if (filePath) node.value = encodeFilePathURI(toRootDir + filePath);
        }));

        return cssGenerate(tree);
    }

    async getDocument(url: string, orCachedPath?: string): Promise<Document> {
        let htmlString: string | null = null;
        if (orCachedPath) {
            try {
                htmlString = await this.readText(orCachedPath);
            } catch {
                // probably doesn't exist
            }
        }

        if (htmlString === null) {
            htmlString = await this.get(url).then((res) => res.text());
        }

        return new DOMParser().parseFromString(htmlString || "", "text/html");
    }
}
