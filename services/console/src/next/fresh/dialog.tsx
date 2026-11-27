import { render } from "solid-js/web";
import type {
	JsonFreshSignIn,
	JsonOAuthUrl,
	JsonProved,
	JsonSignInEmail,
} from "../../types/bencher";
import code from "../auth/code.css?inline";
import { SIGN_IN_FAILED, inboxFailure } from "../auth/copy";
import type { Provider } from "../auth/Door";
import { keepFreshWindow, onProved, signalProved } from "../auth/proved";
import { ask } from "../auth/request";
import FreshDialog, { type FreshEffects } from "./FreshDialog";

/** The code step's styles, which only this dialog brings to the console, when it opens. */
let styled = false;

/** The dialog open now, which every change that needs a fresh sign-in meanwhile waits on. */
let open: Promise<boolean> | undefined;

/**
 * Ask the reader to sign in again, over the page, and resolve whether they did:
 * by a code here, or by a link or a provider's window in another tab.
 */
export const signInAgain = () => {
	open ??= openDialog().finally(() => {
		open = undefined;
	});
	return open;
};

const openDialog = () =>
	new Promise<boolean>((resolve) => {
		if (!styled) {
			const sheet = new CSSStyleSheet();
			sheet.replaceSync(code);
			document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
			styled = true;
		}
		const root = document.createElement("div");
		document.body.append(root);
		let done = false;
		const dispose = render(
			() => (
				<FreshDialog
					effects={EFFECTS}
					onDone={(proved) => {
						if (done) {
							return;
						}
						done = true;
						dispose();
						root.remove();
						resolve(proved);
					}}
				/>
			),
			root,
		);
	});

const EFFECTS: FreshEffects = {
	methods: () => ask<JsonFreshSignIn>("GET", "/v0/auth/fresh"),
	email: () => ask<JsonSignInEmail>("POST", "/v0/auth/fresh/email"),
	resend: () => ask<JsonSignInEmail>("POST", "/v0/auth/fresh/email/resend"),
	prove: async (entered) => {
		const answer = await ask<JsonProved>("POST", "/v0/auth/fresh/code", {
			code: entered,
		});
		// Every other tab's dialog for this session moves on too.
		if (answer.ok) {
			signalProved(localStorage);
		}
		return answer;
	},
	provider: async (provider: Provider) => {
		// Opened before anything is awaited, while the press still lets a page open a window.
		const popup = window.open(
			"",
			"bencher-fresh",
			"popup,width=520,height=720",
		);
		if (!popup) {
			return SIGN_IN_FAILED;
		}
		const answer = await ask<JsonOAuthUrl>(
			"POST",
			`/v0/auth/fresh/providers/${provider}/start`,
		);
		if (!answer.ok) {
			popup.close();
			return inboxFailure(answer.refused);
		}
		keepFreshWindow(popup.sessionStorage);
		popup.location.replace(answer.data.url);
		return undefined;
	},
	onProved,
};
