import { GOOGLE_RECAPTCHA_SITE_KEY } from "astro:env/client";

export const loadRecaptcha = async () => {
	if (!GOOGLE_RECAPTCHA_SITE_KEY) return false;
	if (typeof window === "undefined") return false;
	// @ts-ignore
	if (window.grecaptcha?.execute) return true;
	try {
		await new Promise<void>((resolve, reject) => {
			const script = document.createElement("script");
			script.src = `https://www.google.com/recaptcha/api.js?render=${GOOGLE_RECAPTCHA_SITE_KEY}`;
			script.async = true;
			script.onload = () => resolve();
			script.onerror = () => reject(new Error("Failed to load reCAPTCHA"));
			document.head.appendChild(script);
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
	console.log("Getting reCAPTCHA token for action:", action);
	if (!GOOGLE_RECAPTCHA_SITE_KEY) return null;
	console.log("GOOGLE_RECAPTCHA_SITE_KEY is set");
	const ok = await loadRecaptcha();
	if (!ok) return null;
	console.log("reCAPTCHA is loaded");
	// @ts-ignore
	if (!window.grecaptcha) return null;
	console.log("window.grecaptcha is available");
	try {
		// @ts-ignore
		await new Promise<void>((r) => window.grecaptcha.ready(r));
		// @ts-ignore
		return window.grecaptcha.execute(GOOGLE_RECAPTCHA_SITE_KEY, { action });
	} catch (e) {
		console.error("Error getting reCAPTCHA token:", e);
		return null;
	}
};
