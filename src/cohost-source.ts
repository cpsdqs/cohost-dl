import { CohostContext } from "./context.ts";

// a sample post we'll be using for this.
// edge case that I will not be handling: user has blocked staff
const SAMPLE_POST_URL = "https://cohost.org/staff/post/7611443-cohost-to-shut-down";

interface ISourceMap {
    version: 3;
    file: string;
    mapping: string;
    sources: string[];
    sourcesContent: string[];
    names: string[];
}

/** Loads cohost frontend source and returns root path */
export async function loadCohostSource(ctx: CohostContext): Promise<string> {
    const filePath = await ctx.loadResourceToFile(SAMPLE_POST_URL);
    const document = await ctx.getDocument(SAMPLE_POST_URL, filePath ?? undefined);

    const varsScript = document.getElementById("env-vars");
    if (!varsScript) throw new Error('missing <script id="env-vars">');
    const vars = JSON.parse(varsScript.innerHTML);

    const siteVersion = vars.VERSION as string;

    // TODO: use cached

    console.log(`~~ cohost source version ${siteVersion}`);
    const rootDir = `~src/${siteVersion}`;

    const scriptFileSources = new Set<string>();

    for (const script of document.querySelectorAll('script[data-chunk]')) {
        const src = script.getAttribute('src');
        if (!src?.endsWith('.js')) continue;
        const resolvedSrc = new URL(src, SAMPLE_POST_URL);
        scriptFileSources.add(resolvedSrc.toString());
    }

    for (const link of document.querySelectorAll('link[rel="preload"][as="script"]')) {
        const href = link.getAttribute('href');
        if (!href?.endsWith('.js')) continue;
        const resolvedSrc = new URL(href, SAMPLE_POST_URL);
        scriptFileSources.add(resolvedSrc.toString());
    }

    const loadScript = async (src: string) => {
        await ctx.loadResourceToFile(src);
        const sourceMapURL = src.toString() + '.map';
        const sourceMapPath = await ctx.loadResourceToFile(sourceMapURL);

        if (sourceMapPath) {
            const sourceMap = (await ctx.readJson(sourceMapPath)) as ISourceMap;

            if (sourceMap.sourcesContent) {
                for (let i = 0; i < sourceMap.sources.length; i++) {
                    const url = new URL(sourceMap.sources[i]);
                    const content = sourceMap.sourcesContent[i];

                    if (!content) continue;

                    const urlPath = decodeURI(url.pathname).replace(/^\/+/, '');
                    const filePath = `${rootDir}/${urlPath}`;

                    await ctx.write(filePath, content);
                }
            }
        }
    }

    for (const src of scriptFileSources) {
        await loadScript(src);
    }

    // discover chunk list
    const allOtherScripts: string[] = [];
    {
        const chunkIndexScript = await ctx.readText(`${rootDir}/webpack/runtime/get javascript chunk filename`);

        const NAMES_START = '"" + ({';
        const namesStart = chunkIndexScript.indexOf(NAMES_START);
        const namesWithTrailing = chunkIndexScript.substring(namesStart + NAMES_START.length);
        const namesStr = namesWithTrailing.substring(0, namesWithTrailing.indexOf('}'));

        const names = JSON.parse(`{${namesStr}}`);

        const HASHES_START = ') + "." + {';
        const hashesStart = namesWithTrailing.indexOf(HASHES_START);
        const hashesWithTrailing = namesWithTrailing.substring(hashesStart + HASHES_START.length);
        const hashesStr = hashesWithTrailing.substring(0, hashesWithTrailing.indexOf('}'));

        const hashes = JSON.parse(`{${hashesStr}}`);

        for (const k of Object.keys(hashes)) {
            allOtherScripts.push(`https://cohost.org/static/${names[k] ?? k}.${hashes[k]}.js`);
        }
    }

    for (const src of allOtherScripts) {
        await loadScript(src);
    }

    return rootDir;
}
