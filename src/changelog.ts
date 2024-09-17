import * as path from "jsr:@std/path";

async function checkForUpdatesImpl() {
    const currentChangelog = await Deno.readTextFile(path.join(import.meta.dirname, "../CHANGELOG.txt"));

    const changelogLines = currentChangelog.replace(/\r\n/g, '\n').split("\n");
    const url = changelogLines.shift().split("url=")[1];

    const newestChangelogRes = await fetch(url);
    if (!newestChangelogRes.ok) {
        throw new Error(`could not fetch update information: ${await newestChangelogRes.text()}`);
    }
    const newestChangelog = await newestChangelogRes.text();
    const newLines = newestChangelog.split("\n");
    newLines.shift();

    const firstNonEmptyLine = changelogLines.find(item => !!item);
    const firstNewNonEmptyLine = newLines.find(item => !!item);

    const latestVersion = newLines.find(line => line.startsWith('*'));
    const thisVersion = changelogLines.find(line => line.startsWith('*'));

    if (latestVersion === thisVersion) {
        return;
    }

    console.error('\x1b[32m=== cohost-dl update found ===\x1b[m');
    console.error('maybe you want to update your downloaded version.');
    console.error('changes: ');

    for (const line of newLines) {
        if (line === thisVersion) break;

        console.error(line);
    }

    console.error('\x1b[32m=== * ===\x1b[m');
}

export async function checkForUpdates() {
    try {
        await checkForUpdatesImpl();
    } catch (error) {
        console.error(`error checking for updates: ${error}`);
    }
}
