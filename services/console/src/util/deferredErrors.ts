/**
 * Keep the errors a page raises until the error reporter arrives, then hand
 * them to it once and stop listening.
 */
export const deferErrors = (
	target: Pick<Window, "addEventListener" | "removeEventListener">,
) => {
	const queued: unknown[] = [];
	const onError = (event: ErrorEvent) => {
		queued.push(event.error ?? event.message);
	};
	const onRejection = (event: PromiseRejectionEvent) => {
		queued.push(event.reason);
	};
	target.addEventListener("error", onError);
	target.addEventListener("unhandledrejection", onRejection);
	return {
		flush: (capture: (error: unknown) => void) => {
			target.removeEventListener("error", onError);
			target.removeEventListener("unhandledrejection", onRejection);
			for (const error of queued.splice(0)) {
				capture(error);
			}
		},
	};
};

/** Run `work` once the page has loaded and the browser is idle. */
export const whenIdle = (work: () => void) => {
	const idle = () =>
		"requestIdleCallback" in window
			? requestIdleCallback(() => work(), { timeout: 2_000 })
			: setTimeout(work, 0);
	if (document.readyState === "complete") {
		idle();
	} else {
		window.addEventListener("load", idle, { once: true });
	}
};
