import { readFileSync, statSync } from "node:fs";
import { expect, seed } from "./fixtures";

/** The link in the newest email the API sent to `email` with `subject`. */
export const mailedLink = async (email: string, subject: string) => {
	let link: undefined | URL;
	await expect
		.poll(() => {
			link = newestLink(email, subject);
			return link;
		})
		.toBeDefined();
	return link as URL;
};

/** The newest email the API sent to `email` with `subject`, whole. */
export const mailed = async (email: string, subject: string) => {
	let found: undefined | string;
	await expect
		.poll(() => {
			found = newest(email, subject);
			return found;
		})
		.toBeDefined();
	return found as string;
};

/** Where the API's log ends now, so a read from it sees only the emails sent after. */
export const mailMark = () => statSync(seed.api_log).size;

/** The code and the link in the newest sign in email the API sent to `email`, after `mark`. */
export const mailedSignIn = async (email: string, mark = 0) => {
	let signIn: undefined | { code: string; link: URL };
	await expect
		.poll(() => {
			signIn = newestSignIn(email, mark);
			return signIn;
		})
		.toBeDefined();
	return signIn as { code: string; link: URL };
};

/** The code in the newest sign in email the API sent to `email`, after `mark`. */
export const mailedCode = async (email: string, mark = 0) =>
	(await mailedSignIn(email, mark)).code;

// The API logs each email it sends as one JSON line, its first link the email's button.
const newestLink = (email: string, subject: string) => {
	const url = newest(email, subject)?.match(/https?:\/\/\S+/)?.[0];
	return url ? new URL(url) : undefined;
};

const newest = (email: string, subject: string) => {
	const lines = readFileSync(seed.api_log, "utf8").split("\n").reverse();
	for (const line of lines) {
		const msg = message(line);
		if (msg?.includes(`<${email}>`) && msg.includes(`Subject: ${subject}\n`)) {
			return msg;
		}
	}
	return undefined;
};

const newestSignIn = (email: string, mark: number) => {
	const lines = readFileSync(seed.api_log)
		.subarray(mark)
		.toString("utf8")
		.split("\n")
		.reverse();
	for (const line of lines) {
		const msg = message(line);
		const code = msg?.includes(`<${email}>`)
			? msg.match(/^Subject: Your Bencher code is (\d{6})$/m)?.[1]
			: undefined;
		const link = msg?.match(/https?:\/\/\S+/)?.[0];
		if (code && link) {
			return { code, link: new URL(link) };
		}
	}
	return undefined;
};

const message = (line: string): undefined | string => {
	try {
		const msg = JSON.parse(line)?.msg;
		return typeof msg === "string" ? msg : undefined;
	} catch {
		return undefined;
	}
};
