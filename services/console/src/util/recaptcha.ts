import { RECAPTCHA_SITE_KEY } from "astro:env/client";
import { theme } from "../components/navbar/theme/util";

let recaptchaWidgetId: number | null = null;

export const loadRecaptcha = async () => {
	if (!RECAPTCHA_SITE_KEY) return false;
	if (typeof window === "undefined") return false;
	// @ts-expect-error
	if (window.grecaptcha?.render) return true;
	try {
		await new Promise<void>((resolve, reject) => {
			const script = document.createElement("script");
			script.src = "https://www.google.com/recaptcha/api.js?render=explicit";
			script.async = true;
			script.onload = () => resolve();
			script.onerror = () => reject(new Error("Failed to load reCAPTCHA"));
			document.head.appendChild(script);
		});

		// @ts-expect-error
		await new Promise<void>((r) => window.grecaptcha?.ready(r));

		// Create container element
		let container = document.getElementById("recaptcha-container");
		if (!container) {
			container = document.createElement("div");
			container.id = "recaptcha-container";
			document.body.appendChild(container);
		}

		// https://developers.google.com/recaptcha/docs/faq#can-i-customize-the-recaptcha-widget-or-badge
		// https://developers.google.com/recaptcha/docs/invisible#js_api
		// @ts-expect-error
		recaptchaWidgetId = window.grecaptcha.render(container, {
			sitekey: RECAPTCHA_SITE_KEY,
			size: "invisible",
			theme: theme(),
		});
	} catch (e) {
		console.error("Error loading reCAPTCHA:", e);
		return false;
	}
	return true;
};

export const getRecaptchaToken = async (
	action: string,
): Promise<string | null> => {
	if (!RECAPTCHA_SITE_KEY) return null;
	const ok = await loadRecaptcha();
	if (!ok) return null;
	// @ts-expect-error
	if (!window.grecaptcha) return null;
	try {
		// @ts-expect-error
		await new Promise<void>((r) => window.grecaptcha.ready(r));
		// @ts-expect-error
		return window.grecaptcha.execute(recaptchaWidgetId, { action });
	} catch (e) {
		console.error("Error getting reCAPTCHA token:", e);
		return null;
	}
};
