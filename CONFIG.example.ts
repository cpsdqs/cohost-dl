// copy the cookie header from dev tools on cohost.org. this is used to log in, so don't share it
export const COOKIE = 'connect.sid=adhjsakfahdsfjkash';

// load all of your own posts
export const PROJECTS = ['your-handle'];

// load some specific additional posts
export const POSTS = [
    'https://cohost.org/example/123456-example-post',
];

// some CSS posts contain external images that load forever
export const DO_NOT_FETCH_HOSTNAMES = [
    'eggbugpocket.queertra.sh', // GIF plays Pokémon
    'r0t.is', // Cohost runs Windows XP
];

// some posts may have disappeared between loading the list of posts and actually loading the posts,
// and give you a '404 not found' error. so, these post IDs can be listed here and be skipped when loading,
// so as not to keep retrying every time you run the script
export const SKIP_POSTS = [
    9639936,
];

// You can keep this set to '' if you don't have a data portability archive from cohost.
// If you do have one, set this to the path to the directory that contains the `user.json` file.
// e.g. if you have it at /Users/example/Desktop/cohost-data/user.json,
// then set this to '/Users/example/Desktop/cohost-data'.
// This information will then be used to also load posts you've commented on or sent an ask for.
export const DATA_PORTABILITY_ARCHIVE_PATH = '';

// Set this to false to disable Javascript, which is responsible for interaction on the generated pages
// (read more/read less, opening/closing CWs, image attachments, etc.).
// It's a little janky, so maybe you want an HTML-only export.
export const ENABLE_JAVASCRIPT = true;

// Alters pages to look like they're being viewed by a more generic observer instead of how your account sees things.
// - Attempts to revert settings for silenced tags, CWs, 18+
//   - These cannot be completely removed right now. Original settings will be briefly visible if they were applicable
//     to that particular post.
// - Reverts to the default theme
// - Removes bookmarked tags, private notes, private contact info, the page switcher, whether you liked a post,
//   and whether you were following someone.
// - Does not remove your own handle from some internal data
// - Does not hide posts from private accounts
//
// NOTE: currently breaks Javascript on all of your own post pages
export const GENERIC_OBSERVER = false;
