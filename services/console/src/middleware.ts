import { defineMiddleware } from "astro:middleware";

/** Every auth page is never cached, and never names itself to another site as a referrer. */
export const onRequest = defineMiddleware(async (context, next) => {
	const response = await next();
	if (!isAuthPage(context.url.pathname)) {
		return response;
	}
	// A redirect's headers cannot change, so the answer is made again.
	const headers = new Headers(response.headers);
	headers.set("Referrer-Policy", "no-referrer");
	headers.set("Cache-Control", "no-store");
	return new Response(response.body, {
		status: response.status,
		statusText: response.statusText,
		headers,
	});
});

export const isAuthPage = (pathname: string) =>
	pathname === "/auth" || pathname.startsWith("/auth/");
