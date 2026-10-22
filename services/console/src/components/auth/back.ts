import { decodeBase64 } from "../../util/convert";
import { isWebUrl, sameOriginUrl } from "../../util/url";
import { consentPath } from "./authorize/path";

/** The page a sign in returns to, kept where a magic link opened in another tab of this browser reads it. */
const RETURN_KEY = "BENCHER_RETURN";
/** As long as a magic link lives, so a page kept for one sign in never returns a later one. */
export const RETURN_TTL = 30 * 60 * 1000;

// The pages that send a reader to sign in: the console and the public plot.
const RETURNS = ["/console/", "/perf/", "/next/console/", "/next/perf/"];
// Servers resolve these differently, and no page of the console or the public plot has one.
const UNRESOLVED = /\\|(^|\/)\.\.?(\/|$)/;
// Sign in and its flows, the build's own files, and any file that is not a page.
const NOT_A_PAGE = /^[/\\]+(auth([/\\]|$)|_)|\.[^/\\]*$/i;

/** The page that a `back` parameter names, absolute on `origin`, or nothing for another origin, scheme, or anything but a page. */
export const backPage = (back: unknown, origin: string): undefined | string => {
	if (typeof back !== "string" || !back) {
		return undefined;
	}
	try {
		return page(decodeBase64(back), origin);
	} catch {
		return undefined;
	}
};

const page = (url: unknown, origin: string): undefined | string => {
	const href = typeof url === "string" ? sameOriginUrl(url, origin) : undefined;
	if (!href || !isWebUrl(href)) {
		return undefined;
	}
	const path = decoded(new URL(href).pathname);
	return path !== undefined &&
		!UNRESOLVED.test(path) &&
		(path === "/console" ||
			RETURNS.some((prefix) => path.startsWith(prefix))) &&
		!NOT_A_PAGE.test(path)
		? href
		: undefined;
};

// Decoded until it settles, since a server may decode a path more than once.
const decoded = (path: string): undefined | string => {
	let current = path;
	for (let round = 0; round < 4; round++) {
		const next = decodeURIComponent(current);
		if (next === current) {
			return current;
		}
		current = next;
	}
	return undefined;
};

/** Keep the page that `back` names for the sign in this page starts, or forget any page kept before. */
export const keepReturn = (
	storage: Pick<Storage, "setItem" | "removeItem">,
	back: unknown,
	origin: string,
	now: number,
) => {
	const href = backPage(back, origin);
	try {
		if (href) {
			storage.setItem(RETURN_KEY, JSON.stringify({ href, at: now }));
		} else {
			storage.removeItem(RETURN_KEY);
		}
	} catch {
		// A browser that keeps nothing returns the reader to their main project.
	}
};

/** The kept page, checked again since anything can write storage, and forgotten either way. */
export const takeReturn = (
	storage: Pick<Storage, "getItem" | "removeItem">,
	origin: string,
	now: number,
): undefined | string => {
	try {
		const value = storage.getItem(RETURN_KEY);
		storage.removeItem(RETURN_KEY);
		const kept = JSON.parse(value ?? "null");
		const age = now - kept?.at;
		return age >= 0 && age <= RETURN_TTL ? page(kept.href, origin) : undefined;
	} catch {
		return undefined;
	}
};

/** Keep the way back this sign in page was opened with. */
export const keepBack = (back: unknown) =>
	keepReturn(localStorage, back, location.origin, Date.now());

// Every part of a page that finishes a sign in goes to the same place, though the first takes the kept page.
let taken: undefined | { href: undefined | string };

/** Where a finished sign in goes: a pending consent, then the kept page, else nothing, for the main project. */
export const signedInPath = (authorization: unknown): undefined | string => {
	taken ??= { href: takeReturn(localStorage, location.origin, Date.now()) };
	return consentPath(authorization) ?? taken.href;
};
