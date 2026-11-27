import { REQUEST_PARAM, requestUuid } from "./path";

const ERROR_PARAM = "error";
/** The Bencher CLI's client ID, which the server knows without a document. */
export const BENCHER_CLI = "bencher_cli";
const FRESH_SIGN_IN = "fresh_sign_in";
const NOT_A_SERVER_ADMIN = "not_a_server_admin";
const USER_KEY_PREFIX = "bencher_user_";
const PROJECT_KEY_PREFIX = "bencher_run_";

/** What the consent page was asked to show: one request, or an error. */
export type ConsentRequest = { uuid: string } | { error: ConsentError };

/** The error codes the API sends the consent page, plus the console's own. */
export enum ConsentError {
	InvalidRequest = "invalid_request",
	InvalidClient = "invalid_client",
	TemporarilyUnavailable = "temporarily_unavailable",
	InvalidRedirectUri = "invalid_redirect_uri",
	/** Any other code, which is never shown. */
	Other = "other",
	/** A link without exactly one valid request. */
	Malformed = "malformed",
}

/** Read the consent page's query: only `request` or `error`, never shown as given. */
export const consentRequest = (params: URLSearchParams): ConsentRequest => {
	const errors = params.getAll(ERROR_PARAM);
	if (errors.length > 0) {
		return {
			error: errors.length === 1 ? consentError(errors[0]) : ConsentError.Other,
		};
	}
	const requests = params.getAll(REQUEST_PARAM);
	const uuid = requests.length === 1 ? requestUuid(requests[0]) : undefined;
	return uuid ? { uuid } : { error: ConsentError.Malformed };
};

const consentError = (code: undefined | string): ConsentError => {
	switch (code) {
		case ConsentError.InvalidRequest:
		case ConsentError.InvalidClient:
		case ConsentError.TemporarilyUnavailable:
		case ConsentError.InvalidRedirectUri:
			return code;
		default:
			return ConsentError.Other;
	}
};

export const consentErrorText = (error: ConsentError): string => {
	switch (error) {
		case ConsentError.InvalidRequest:
			return "The app's request to connect was not valid.";
		case ConsentError.InvalidClient:
			return "Bencher could not verify the app that asked to connect.";
		case ConsentError.TemporarilyUnavailable:
			return "Bencher is too busy to take the app's request right now. Wait a minute and try again.";
		case ConsentError.InvalidRedirectUri:
			return "The app asked to send you somewhere it has not registered.";
		case ConsentError.Other:
			return "The app's request to connect could not be completed.";
		case ConsentError.Malformed:
			return "This link to connect an app is not valid.";
	}
};

/** What a pasted API key is. */
export enum PastedKey {
	User = "user",
	Project = "project",
	Other = "other",
}

export const pastedKey = (key: string): PastedKey => {
	if (key.startsWith(USER_KEY_PREFIX)) {
		return PastedKey.User;
	}
	if (key.startsWith(PROJECT_KEY_PREFIX)) {
		return PastedKey.Project;
	}
	return PastedKey.Other;
};

/** The credential an answer was sent with. */
export enum Credential {
	Session = "session",
	Key = "key",
}

/** Why the API would not show or answer a request. */
export enum Refusal {
	/** No such request: unknown, expired, denied, or apps cannot connect to this server. */
	Gone = "gone",
	/** The request was already answered. */
	Answered = "answered",
	Busy = "busy",
	/** The session is no longer accepted, so the user signs in again. */
	SessionEnded = "session_ended",
	KeyRefused = "key_refused",
	/** The session must sign in again to approve the Bencher CLI. */
	FreshSignIn = "fresh_sign_in",
	/** Only a server admin can approve the Bencher CLI as admin. */
	NotAdmin = "not_admin",
	/** Anything else, the network included. */
	Failed = "failed",
}

export const refusal = (
	status: undefined | number,
	credential: undefined | Credential,
	/** The API's `error_code`, if it gave one. */
	code?: string,
): Refusal => {
	if (status === 403 && credential === Credential.Session) {
		if (code === FRESH_SIGN_IN) {
			return Refusal.FreshSignIn;
		}
		if (code === NOT_A_SERVER_ADMIN) {
			return Refusal.NotAdmin;
		}
	}
	switch (status) {
		case 404:
			return Refusal.Gone;
		case 409:
			return Refusal.Answered;
		case 429:
			return Refusal.Busy;
		// A malformed key is a `400`; an ended, unknown, or locked user's session is a `401`.
		case 400:
		case 401:
		case 403:
			if (credential === Credential.Key) {
				return Refusal.KeyRefused;
			}
			if (credential === Credential.Session && status === 401) {
				return Refusal.SessionEnded;
			}
			return Refusal.Failed;
		default:
			return Refusal.Failed;
	}
};

/** Why the API refused, for a page that offers a pasted key unless `keyed` is false. */
export const refusalText = (refusal: Refusal, keyed = true): string => {
	switch (refusal) {
		case Refusal.Gone:
			return "This request to connect is no longer available. It may have expired, or this server may not connect apps.";
		case Refusal.Answered:
			return "This request to connect was already answered.";
		case Refusal.Busy:
			return "Too many requests. Wait a minute and try again.";
		case Refusal.SessionEnded:
			return keyed
				? "Your session ended. Sign in again, or approve with a user API key."
				: "Your session ended. Sign in again.";
		case Refusal.KeyRefused:
			return "That user API key was not accepted. Check that it is current and that its account is not locked.";
		case Refusal.FreshSignIn:
			return "Sign in again to approve.";
		case Refusal.NotAdmin:
			return "Not a server admin";
		case Refusal.Failed:
			return "Bencher could not be reached. Try again.";
	}
};

/** Where the API sends the browser with the user's answer, when a browser may go there. */
export const answerUrl = (url: unknown): undefined | string => {
	if (typeof url !== "string") {
		return undefined;
	}
	try {
		const { protocol } = new URL(url);
		return protocol === "https:" || protocol === "http:" ? url : undefined;
	} catch {
		return undefined;
	}
};

/** The `error_code` of an API error's body, if it has one. */
export const errorCode = (body: string): undefined | string => {
	try {
		const code = JSON.parse(body)?.error_code;
		return typeof code === "string" ? code : undefined;
	} catch {
		return undefined;
	}
};
