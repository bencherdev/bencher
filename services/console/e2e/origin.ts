/**
 * Whether a request a page sends to the API's own origin strays from the
 * console's: every one does, but for an outside app sending the browser's top
 * frame to the API's OAuth pages.
 */
export const strayToApi = (url: URL, topNavigation: boolean) =>
	!(topNavigation && url.pathname.startsWith("/oauth/"));
