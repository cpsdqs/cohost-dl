import * as data from "../CONFIG.ts";

export const COOKIE = data.COOKIE;

export const POSTS = data.POSTS ?? [];

export const PROJECTS = data.PROJECTS ?? [];

export const SKIP_POSTS = data.SKIP_POSTS ?? [];

export const DATA_PORTABILITY_ARCHIVE_PATH =
    data.DATA_PORTABILITY_ARCHIVE_PATH ?? "";

export const DO_NOT_FETCH_HOSTNAMES = data.DO_NOT_FETCH_HOSTNAMES ?? [];

export const ENABLE_JAVASCRIPT = data.ENABLE_JAVASCRIPT ?? true;

export const GENERIC_OBSERVER = data.GENERIC_OBSERVER ?? false;

export const REQUEST_DELAY_SECS = data.REQUEST_DELAY_SECS ?? 0;
