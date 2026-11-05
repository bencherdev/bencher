import type { APIRoute } from "astro";
import {
	BENCHER_API_URL,
	BENCHER_RELAY_SECRET,
	INTERNAL_API_URL,
} from "astro:env/server";
import { relay } from "../../util/relay";

export const prerender = false;

export const ALL: APIRoute = (context) =>
	relay(context, {
		internalApiUrl: INTERNAL_API_URL,
		apiUrl: BENCHER_API_URL,
		secret: BENCHER_RELAY_SECRET,
	});
