import resolve from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';
import typescript from '@rollup/plugin-typescript';
import replace from '@rollup/plugin-replace';
import alias from '@rollup/plugin-alias';
import json from '@rollup/plugin-json';
import path from 'node:path';

const dirname = path.dirname(new URL(import.meta.url).pathname);


const EMOJI = {
    "chunks.png": "f59b84127fa7b6c48b6c.png",
    "eggbug-classic.png": "41454e429d62b5cb7963.png",
    "eggbug.png": "17aa2d48956926005de9.png",
    "sixty.png": "9a6014af31fb1ca65a1f.png",
    "unyeah.png": "5cf84d596a2c422967de.png",
    "yeah.png": "014b0a8cc35206ef151d.png",
};
const PLUS_EMOJI = {
    "eggbug-asleep.png": "ebbf360236a95b62bdfc.png",
    "eggbug-devious.png": "c4f3f2c6b9ffb85934e7.png",
    "eggbug-heart-sob.png": "b59709333449a01e3e0a.png",
    "eggbug-nervous.png": "d2753b632211c395538e.png",
    "eggbug-pensive.png": "ae53a8b5de7c919100e6.png",
    "eggbug-pleading.png": "11c5493261064ffa82c0.png",
    "eggbug-relieved.png": "3633c116f0941d94d237.png",
    "eggbug-shocked.png": "b25a9fdf230219087003.png",
    "eggbug-smile-hearts.png": "d7ec7f057e6fb15a94cc.png",
    "eggbug-sob.png": "9559ff8058a895328d76.png",
    "eggbug-tuesday.png": "90058099e741e483208a.png",
    "eggbug-uwu.png": "228d3a13bd5f7796b434.png",
    "eggbug-wink.png": "3bc3a1c5272e2ceb8712.png",
    "host-aww.png": "9bb403f3822c6457baf6.png",
    "host-cry.png": "530f8cf75eac87716702.png",
    "host-evil.png": "cb9a5640d7ef7b361a1a.png",
    "host-frown.png": "99c7fbf98de865cc9726.png",
    "host-joy.png": "53635f5fe850274b1a7d.png",
    "host-love.png": "c45b6d8f9de20f725b98.png",
    "host-nervous.png": "e5d55348f39c65a20148.png",
    "host-plead.png": "fa883e2377fea8945237.png",
    "host-shock.png": "bfa6d6316fd95ae76803.png",
    "host-stare.png": "a09d966cd188c9ebaa4c.png",
};

function convertEmoji(map) {
    return Object.fromEntries(Object.entries(map).map(([k, v]) => [k, `/static/${v}`]));
}

const banner = `
process.env.HOME_URL = 'https://cohost.org/';

globalThis.require = {};
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
`;

export default {
    input: 'src/index.tsx',
    output: {
        banner,
        file: 'compiled.js',
    },
    plugins: [
        alias({
            entries: [
                { find: 'stream', replacement: 'readable-stream' },
                { find: 'util', replacement: path.join(dirname, 'src', 'patch_util.js') },
                { find: 'css-tree', replacement: path.join(dirname, 'node_modules', 'css-tree', 'dist', 'csstree.esm.js') },
            ]
        }),
        typescript(),
        json(),
        replace({
            "process.env.NODE_ENV": "'production'",
            preventAssignment: true,
        }),
        resolve({ preferBuiltins: false }),
        commonjs(),
    ],
}
