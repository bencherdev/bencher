import { ApiError } from "../api";

/** What the API said when it refused, or `unanswered` when it never answered. */
export const failureOf = (
	error: unknown,
	refused: string,
	unanswered: string,
) =>
	error instanceof ApiError && error.status !== undefined
		? `${refused}: ${apiMessage(error)}`
		: unanswered;

const apiMessage = (error: ApiError) => {
	try {
		const message = JSON.parse(error.message)?.message;
		if (typeof message === "string" && message) {
			return message;
		}
	} catch {
		// Not the API's JSON; its status says enough.
	}
	return `the API answered ${error.status}.`;
};
