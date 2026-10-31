import "./page.css";
import Button from "@bencherdev/ui/Button";
import inter from "@bencherdev/ui/fonts/inter-latin-wght-normal.woff2?inline";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import type { CallToolResult, Transport } from "@modelcontextprotocol/client";
import {
	App,
	type McpUiTheme,
	PostMessageTransport,
} from "@modelcontextprotocol/ext-apps";
import { batch, createSignal } from "solid-js";
import { render } from "solid-js/web";
import wordmarkDark from "../../../public/wordmark-dark.svg?inline";
import wordmarkLight from "../../../public/wordmark-light.svg?inline";
import type { JsonConsolePerf, JsonProject } from "../../types/bencher";
import Embed, { type EmbedLinks, type PlotView } from "../embed/Embed";
import { alertPath, reportPath } from "../paths";
import { type ExploreQuery, blankQuery, decodeQuery } from "../query/query";

/** What `query_perf` returns: the console's plot, with its project and the link that opens it. */
export type PerfResult = JsonConsolePerf & {
	project: JsonProject;
	url: string;
};

/** Explore's plot of a `query_perf` result, in a host's frame that `transport` talks to. */
export const boot = async (mount: HTMLElement, transport: Transport) => {
	const font = loadInter();
	const app = new App(
		{ name: "Bencher", version: "1.0.0" },
		{ availableDisplayModes: ["inline"] },
		{ autoResize: false },
	);
	const [input, setInput] = createSignal<Record<string, unknown>>();
	const [result, setResult] = createSignal<PerfResult>();
	const [query, setQuery] = createSignal(PENDING);
	const [refused, setRefused] = createSignal<string>();
	const [busy, setBusy] = createSignal(false);
	const [plot, setPlot] = createSignal<PlotView>();
	const links = () => {
		const perf = result();
		return perf && consoleLinks(perf);
	};

	const show = (next: CallToolResult) => {
		const perf = next.structuredContent as PerfResult | undefined;
		if (next.isError || !perf) {
			setRefused(failure(next));
			return;
		}
		batch(() => {
			// A refresh with the same link keeps the lines the reader hid and focused.
			if (perf.url !== result()?.url) {
				setQuery(decodeQuery(new URL(perf.url).search));
			}
			setResult(perf);
			setRefused(undefined);
		});
	};
	const refresh = () => {
		setBusy(true);
		app
			.callServerTool({ name: "query_perf", arguments: input() ?? {} })
			.then(show, () => setRefused(FAILED))
			.finally(() => setBusy(false));
	};
	app.ontoolinput = ({ arguments: args }) => setInput(args);
	app.ontoolresult = show;
	app.ontoolcancelled = () => setRefused(FAILED);
	app.onteardown = () => ({});
	app.onhostcontextchanged = ({ theme }) => {
		if (theme) {
			setTheme(theme);
		}
	};

	let theme: McpUiTheme | undefined;
	// The host sizes an inline frame to the page.
	let stopSizing: (() => void) | undefined;
	try {
		await app.connect(transport);
		theme = app.getHostContext()?.theme;
		stopSizing = app.setupSizeChangedNotifications();
	} catch {
		setRefused(FAILED);
	}
	// The plot reads its colors from the theme's tokens and lays out its text in Inter as it first draws.
	setTheme(theme ?? preferredTheme());
	await font;
	import("../plot/Plot")
		.then(({ default: loaded }) => setPlot(() => loaded))
		.catch(() => setRefused(FAILED));

	// The sandbox may block the frame's own navigation, so every link opens through the host.
	const open = (event: MouseEvent) => {
		const link =
			event.target instanceof Element ? event.target.closest("a") : null;
		if (!link) {
			return;
		}
		event.preventDefault();
		if (links()) {
			app.openLink({ url: link.href }).catch(() => {});
		}
	};
	mount.addEventListener("click", open);
	const dispose = render(
		() => (
			<Embed
				slug={result()?.project.slug ?? named(input())}
				name={result()?.project.name}
				nameless={refused() !== undefined}
				query={query()}
				view="plot"
				answer={result()}
				refused={refused()}
				plot={plot()}
				onQuery={setQuery}
				links={links()}
				wordmarks={WORDMARKS}
				actions={
					<Button
						variant="ghost"
						iconOnly
						aria-label="Refresh"
						disabled={busy() || !input()}
						onClick={refresh}
					>
						<svg
							viewBox="0 0 24 24"
							width="14"
							height="14"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
							stroke-linejoin="round"
							aria-hidden="true"
						>
							<path d="M20 12a8 8 0 1 1-1.07-4" />
							<path d="M19 3v5h-5" />
						</svg>
					</Button>
				}
			/>
		),
		mount,
	);
	return async () => {
		stopSizing?.();
		mount.removeEventListener("click", open);
		dispose();
		await app.close();
	};
};

const REFUSED = "The API refused this query";
const FAILED = "The plot did not load";

// Until a result's link names the query, one of each box holds the plot's frame.
const PENDING: ExploreQuery = {
	...blankQuery(),
	branches: [{ uuid: "" }],
	testbeds: [{ uuid: "" }],
	benchmarks: [""],
	measures: [""],
};

// The frame may load images only from itself, so the wordmarks come inlined.
const WORDMARKS = { dark: wordmarkDark, light: wordmarkLight };

const consoleLinks = ({ url, project }: PerfResult): EmbedLinks => {
	const at = (path: string) => new URL(path, url).href;
	return {
		plot: url,
		report: (uuid) => at(reportPath(project.slug, uuid)),
		alert: (uuid) => at(alertPath(project.slug, uuid)),
	};
};

/** The embed's words for a result with no plot: a status under 500 is the API refusing the query. */
const failure = (result: CallToolResult) => {
	const [first] = result.content;
	const status =
		first?.type === "text" ? Number.parseInt(first.text, 10) : Number.NaN;
	return status >= 400 && status < 500 ? REFUSED : FAILED;
};

/** The project the call named, which names the line when no result says more. */
const named = (input: Record<string, unknown> | undefined) =>
	typeof input?.project === "string" ? input.project : "";

// The frame's CSP may allow no font from a URL, a `data:` one included, so Inter loads from its bytes.
const loadInter = async () => {
	const base64 = inter.slice(inter.indexOf(",") + 1);
	const bytes = Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
	try {
		const face = await new FontFace("Inter", bytes, {
			weight: "100 900",
		}).load();
		document.fonts.add(face);
	} catch {
		// The page draws in the next font of the stack.
	}
};

const setTheme = (theme: McpUiTheme) =>
	document.documentElement.setAttribute(THEME_ATTRIBUTE, theme);

const preferredTheme = (): McpUiTheme =>
	matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";

const mount = document.getElementById("perf");
if (mount) {
	boot(mount, new PostMessageTransport(window.parent, window.parent));
}
