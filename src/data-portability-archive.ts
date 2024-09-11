import * as path from "jsr:@std/path";

interface IDPAItems {
    asks: IDPAAsk[];
    comments: IDPAComment[];
}

interface IDPAAsk {
    content: string;
    anon: boolean;
    askingProject: string;
    respondingProject: string;
    responsePost: string;
}

interface IDPAComment {
    commentId: string;
    postedAt: string;
    body: string;
    post: string;
    inReplyTo: string;
    hidden: boolean;
}

export async function readDataPortabilityArchiveItems(dpaPath: string): Promise<IDPAItems> {
    try {
        const userString = await Deno.readTextFile(
            path.join(dpaPath, "user.json"),
        );
        const user: { userId?: string } = JSON.parse(userString);
        if (!user.userId) {
            throw new Error("no userId");
        }
    } catch (err) {
        console.log(
            `${dpaPath} is probably not a data portability archive directory`,
        );
        throw err;
    }

    const asksDir = path.join(dpaPath, "asks");
    const asks: IDPAAsk[] = [];

    for await (const item of Deno.readDir(asksDir)) {
        if (item.isFile && item.name.endsWith('.json')) {
            const text = await Deno.readTextFile(path.join(asksDir, item.name));
            asks.push(JSON.parse(text) as IDPAAsk);
        }
    }

    const projectsDir = path.join(dpaPath, "project");
    const comments: IDPAComment[] = [];

    for await (const item of Deno.readDir(projectsDir)) {
        if (item.isDirectory) {
            const commentsDir = path.join(projectsDir, item.name, 'comments');

            for await (const item of Deno.readDir(commentsDir)) {
                if (item.isFile && item.name.endsWith('.json')) {
                    const text = await Deno.readTextFile(path.join(commentsDir, item.name));
                    comments.push(JSON.parse(text) as IDPAComment);
                }
            }
        }
    }

    return { asks, comments };
}
