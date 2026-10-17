import type { Event, EventHint, captureFeedback } from "@sentry/astro";

export type Kind = "bug" | "feedback";

/** What a report can attach; every one is ticked until the reader unticks it. */
export type Attached =
	| "page"
	| "project"
	| "browser"
	| "build"
	| "session"
	| "email";

interface Image {
	filename: string;
	contentType: string;
	data: Uint8Array;
}

export interface Form {
	kind: Kind;
	message: string;
	image?: Image | undefined;
	/** The items the reader unticked. */
	off: ReadonlySet<Attached>;
}

/** Who sends the report, from where. */
export interface About {
	url: string;
	/** The project's slug, on a project's page. */
	slug?: string | undefined;
	email: string;
	build: string;
}

type Params = Parameters<typeof captureFeedback>[0];

/** The arguments to Sentry's `captureFeedback` for a form. */
export const feedbackArgs = (form: Form, about: About) => {
	const on = (item: Attached) => !form.off.has(item);
	const params: Params = {
		message: form.message.trim(),
		...(on("page") ? { url: about.url } : {}),
		...(on("email") ? { email: about.email } : {}),
		tags: {
			kind: form.kind,
			...(on("project") && about.slug ? { project: about.slug } : {}),
			...(on("build") ? { build: about.build } : {}),
		},
	};
	const hint: EventHint = form.image ? { attachments: [form.image] } : {};
	return { params, hint };
};

/**
 * Removes from a prepared event what the SDK attaches on its own for each
 * unticked item. It runs as the event leaves, after the SDK has added the
 * trace, so nothing it strips comes back.
 */
export const scrub = (event: Event, off: ReadonlySet<Attached>): Event => {
	const metadata = event.sdkProcessingMetadata as
		| { dynamicSamplingContext?: Record<string, unknown> }
		| undefined;
	const sampling = metadata?.dynamicSamplingContext;
	const headers = event.request?.headers;
	if (off.has("page")) {
		delete event.transaction;
		delete event.request?.url;
		delete headers?.Referer;
		delete sampling?.transaction;
	}
	if (off.has("browser")) {
		delete headers?.["User-Agent"];
		delete event.contexts?.culture;
	}
	if (off.has("build")) {
		delete event.release;
		delete event.environment;
		delete sampling?.release;
		delete sampling?.environment;
	}
	if (off.has("session")) {
		delete event.contexts?.trace;
		delete event.breadcrumbs;
		delete metadata?.dynamicSamplingContext;
	}
	return event;
};

const BROWSERS = [
	["Edge", /Edg\/(\d+)/],
	["Opera", /OPR\/(\d+)/],
	["Firefox", /Firefox\/(\d+)/],
	["Chrome", /Chrome\/(\d+)/],
	["Safari", /Version\/(\d+).*Safari/],
] as const;
const SYSTEMS = [
	["iOS", /iPhone|iPad/],
	["Android", /Android/],
	["macOS", /Mac OS X/],
	["Windows", /Windows/],
	["Linux", /Linux/],
] as const;

/** A user agent as a reader names it, such as "Firefox 131, macOS". */
export const browserOf = (agent: string) => {
	for (const [name, pattern] of BROWSERS) {
		const version = pattern.exec(agent)?.[1];
		if (version) {
			const system = SYSTEMS.find(([, test]) => test.test(agent))?.[0];
			return system ? `${name} ${version}, ${system}` : `${name} ${version}`;
		}
	}
	return agent;
};

export const sizeOf = (bytes: number) =>
	bytes < 1024 * 1024
		? `${Math.max(1, Math.round(bytes / 1024))} KB`
		: `${Math.round((bytes / 1024 / 1024) * 10) / 10} MB`;
