import { defineMiddleware } from "astro:middleware";

/** What every auth and console page allows another site: no frame. */
const CONTENT_SECURITY_POLICY = "frame-ancestors 'none'";

/**
 * No other site frames an auth or a console page, and every auth page is
 * never cached and never names itself to another site as a referrer. Each
 * page under them renders per request, since neither server sends a file
 * through here.
 */
export const onRequest = defineMiddleware(async (context, next) => {
	// Read before a page's rewrite can move it.
	const { pathname } = context.url;
	const response = await next();
	const auth = isAuthPage(pathname);
	if (!auth && !isConsolePage(pathname)) {
		return response;
	}
	// A redirect's headers cannot change, so the answer is made again.
	const headers = new Headers(response.headers);
	headers.set("Content-Security-Policy", CONTENT_SECURITY_POLICY);
	if (auth) {
		headers.set("Referrer-Policy", "no-referrer");
		headers.set("Cache-Control", "no-store");
	}
	return new Response(response.body, {
		status: response.status,
		statusText: response.statusText,
		headers,
	});
});

export const isAuthPage = (pathname: string) =>
	pathname === "/auth" || pathname.startsWith("/auth/");

const isConsolePage = (pathname: string) =>
	pathname === "/console" || pathname.startsWith("/console/");
