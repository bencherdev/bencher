import { captureFeedback, getClient, getReplay } from "@sentry/astro";
import { type About, type Form, feedbackArgs, scrub } from "./report";

/** How long a report waits for Sentry, which the console loads once the page is idle. */
const LOAD_WAIT = 10_000;
const POLL = 100;
const SEND_WAIT = 30_000;

/**
 * Hands the report to Sentry user feedback, rejecting when it was not delivered;
 * it waits on the answer itself, since `sendFeedback` would pull the SDK's
 * feedback widget into the Sentry chunk.
 */
export const send = async (form: Form, about: About) => {
	for (let waited = 0; !getClient() && waited < LOAD_WAIT; waited += POLL) {
		await new Promise((resolve) => setTimeout(resolve, POLL));
	}
	const client = getClient();
	if (!client) {
		throw new Error("Sentry did not load");
	}
	// The replay names, and in buffer mode uploads, any feedback sent while it runs.
	const replay = form.off.has("session") ? getReplay() : undefined;
	const mode = replay?.getRecordingMode();
	await replay?.stop();
	const { params, hint } = feedbackArgs(form, about);
	let id: string | undefined;
	return new Promise<void>((resolve, reject) => {
		// The SDK may prepare the event before `captureFeedback` returns its id.
		const stopScrub = client.on("beforeSendEvent", (event) => {
			if (event.type === "feedback" && id === undefined) {
				id = event.event_id;
				scrub(event, form.off);
			}
		});
		const stopWait = client.on("afterSendEvent", (event, response) => {
			if (event.event_id === id) {
				finish(response.statusCode);
			}
		});
		const timer = setTimeout(() => finish(), SEND_WAIT);
		const finish = (status?: number) => {
			stopScrub();
			stopWait();
			clearTimeout(timer);
			if (mode === "session") {
				replay?.start();
			} else if (mode === "buffer") {
				replay?.startBuffering();
			}
			if (status !== undefined && status >= 200 && status < 300) {
				resolve();
			} else {
				reject(new Error(`Sentry answered ${status ?? "nothing"}`));
			}
		};
		captureFeedback(params, hint);
	});
};
