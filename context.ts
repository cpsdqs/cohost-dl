import * as path from "jsr:@std/path";
import { Document, DOMParser } from "jsr:@b-fuze/deno-dom";
import {
    generate as cssGenerate,
    parse as cssParse,
    walk as cssWalk,
} from "npm:css-tree@2.3.1";
import { DO_NOT_FETCH_HOSTNAMES } from "./CONFIG.ts";

const USER_AGENT = "cohost-dl/1.0";

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
                filePath: `rc/external/${url.host}${url.pathname}${url.search}`,
                canFail: true,
            };
        } else {
            console.error(`IGNORING URL ${urlString}`);
        }

        return null;
    }

    pendingResources = new Map<string, Promise<string | null>>();

    async hasFile(filePath: string): Promise<boolean> {
        const fullPath = path.join(this.rootDir, filePath);
        try {
            await Deno.stat(fullPath);
            return true;
        } catch {
            return false;
        }
    }

    async readJson(filePath: string): Promise<object> {
        const fullPath = path.join(this.rootDir, filePath);

        return JSON.parse(await Deno.readTextFile(fullPath));
    }

    async write(filePath: string, data: string | Uint8Array) {
        const fullPath = path.join(this.rootDir, filePath);

        await Deno.mkdir(path.dirname(fullPath), { recursive: true });
        await Deno.writeFile(
            fullPath,
            typeof data === "string" ? new TextEncoder().encode(data) : data,
        );
    }

    async loadResourceToFile(urlString: string): Promise<string | null> {
        const props = this.propsForResourceURL(urlString);
        if (!props) return null;

        const fullPath = path.join(this.rootDir, props.filePath);

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

    async getDocument(url: string): Promise<Document> {
        const htmlString = await this.get(url).then((res) => res.text());
        return new DOMParser().parseFromString(htmlString, "text/html");
    }
}
