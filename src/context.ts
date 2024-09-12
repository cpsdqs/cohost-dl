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

    while (new TextEncoder().encode(filename).byteLength > MAX_FILE_NAME_LENGTH_UTF8) {
        let firstBit = '';
        let rest = filename;
        while (new TextEncoder().encode(firstBit).byteLength < MAX_FILE_NAME_LENGTH_UTF8) {
            const item = rest[Symbol.iterator]().next().value;
            firstBit += item;
            rest = rest.substring(item.length);
        }

        dirname = path.join(dirname, firstBit);
        filename = rest;
    }

    return path.join(dirname, filename);
}

export const POST_URL_REGEX = /^https:\/\/cohost[.]org\/([^\/]+)\/post\/(\d+)-/;

export class CohostContext {
    /** cookie header */
    cookie: string;

    /** output directory for files */
    rootDir: string;

    constructor(cookie: string, rootDir: string) {
        this.cookie = cookie;
        this.rootDir = rootDir;
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

    propsForResourceURL(
        urlString: string,
    ): { fetch: string; filePath: string; canFail?: boolean } | null {
        const url = new URL(urlString);
        if (
            url.hostname === "staging.cohostcdn.org" &&
            url.pathname.match(/^\/[a-z]+\//)
        ) {
            return {
                fetch: `https://staging.cohostcdn.org${url.pathname}`,
                filePath: `rc${decodeURIComponent(url.pathname)}`,
            };
        } else if (url.hostname === "cohost.org") {
            return {
                fetch: urlString,
                filePath: decodeURIComponent(url.pathname.substring(1)), // no leading /
            };
        } else if (url.protocol === "https:" && !DO_NOT_FETCH_HOSTNAMES.includes(url.hostname)) {
            return {
                fetch: urlString,
                filePath: splitTooLongFileName(`rc/external/${url.host}${url.pathname}${url.search}`),
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
            const cleanFilePath = filePath.replace(/[?%*:|"<>]/g, '-');
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

    async getCachedFileForPostURL(url: string): Promise<string | null> {
        const match = url.match(POST_URL_REGEX);
        if (!match) return null;
        const [, projectHandle, id] = match;

        const projectDir = path.join(this.getCleanPath(projectHandle), 'post');
        try {
            for await (const item of Deno.readDir(projectDir)) {
                if (item.name.startsWith(id + '-') && item.name.endsWith('.html')) {
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

        const fullPath = this.getCleanPath(props.filePath);

        try {
            await Deno.stat(fullPath);
            return props.filePath;
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

                let data: string | Uint8Array;
                if (
                    res.headers.get("content-type")?.split(";")?.[0] ===
                        "text/css"
                ) {
                    data = await this.processCss(
                        urlString,
                        props.filePath,
                        await res.text(),
                    );
                } else {
                    data = new Uint8Array(await res.arrayBuffer());
                }

                await this.write(props.filePath, data);

                return props.filePath;
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
            if (filePath) node.value = encodeURI(toRootDir + filePath);
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
