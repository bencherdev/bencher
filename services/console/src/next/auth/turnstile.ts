/** Cloudflare's script, told to draw only the widgets the page renders. */
export const TURNSTILE_SCRIPT =
	"https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";

export interface Turnstile {
	render(
		container: HTMLElement,
		options: {
			sitekey: string;
			execution: "execute";
			callback: (token: string) => void;
			"error-callback": () => boolean;
			"expired-callback": () => void;
		},
	): string | undefined;
	execute(widget: string): void;
	remove(widget: string): void;
}

let loading: Promise<Turnstile> | undefined;

/** Load Cloudflare's script once; a page that will ask for a token starts it as it loads. */
export const loadTurnstile = (): Promise<Turnstile> => {
	loading ??= new Promise<Turnstile>((resolve, reject) => {
		const script = document.createElement("script");
		script.src = TURNSTILE_SCRIPT;
		script.async = true;
		script.onload = () => {
			const turnstile = Reflect.get(window, "turnstile") as
				| Turnstile
				| undefined;
			if (turnstile) {
				resolve(turnstile);
			} else {
				reject(new Error("Turnstile did not start"));
			}
		};
		script.onerror = () => reject(new Error("Turnstile did not load"));
		document.head.append(script);
	}).catch((error: unknown) => {
		// A later try loads it again.
		loading = undefined;
		throw error;
	});
	return loading;
};

/**
 * A new token from an invisible widget, which draws nothing. Each token is
 * good for one request, so every request asks for its own.
 */
export const turnstileToken = async (
	siteKey: string,
	load: () => Promise<Turnstile> = loadTurnstile,
): Promise<string> => {
	const turnstile = await load();
	const container = document.createElement("div");
	document.body.append(container);
	let widget: string | undefined;
	const done = () => {
		if (widget !== undefined) {
			turnstile.remove(widget);
		}
		container.remove();
	};
	try {
		return await new Promise<string>((resolve, reject) => {
			widget = turnstile.render(container, {
				sitekey: siteKey,
				execution: "execute",
				callback: resolve,
				"error-callback": () => {
					reject(new Error("Turnstile refused"));
					// Handled here, so the widget does not retry on its own.
					return true;
				},
				"expired-callback": () => reject(new Error("Turnstile expired")),
			});
			if (widget === undefined) {
				reject(new Error("Turnstile drew no widget"));
				return;
			}
			turnstile.execute(widget);
		});
	} finally {
		done();
	}
};
