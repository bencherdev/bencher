import { createEffect, createSignal, on } from "solid-js";
import { render } from "solid-js/web";
import type { JsonConsolePerf, JsonProject } from "../../types/bencher";
import { embedCode } from "../embed/code";
import {
	drawable,
	perfSearch,
	refusalOf,
	requestKey,
} from "../explore/request";
import type { Copied } from "../explore/Share";
import { handOff } from "../handoff";
import { rememberVersion } from "../memory";
import { type ExploreQuery, decodeQuery, encodeQuery } from "../query/query";
import { isCurrent, readReader, signInHref } from "../reader";
import Public, { type PlotView } from "./Public";
import { publicPlotPath } from "./query";
import { type Refusal, reads } from "./read";

export const boot = (mount: HTMLElement) => {
	const { apiUrl = "", slug = "" } = mount.dataset;
	const reader = readReader(localStorage);
	const token =
		reader && isCurrent(reader, Date.now()) ? reader.token : undefined;
	// Read as no one first, so a public project draws the same for everyone;
	// only a project that refuses everyone is read again as the reader.
	let get = reads(apiUrl);
	const project = `/v0/projects/${encodeURIComponent(slug)}`;

	const initial = decodeQuery(location.search);
	const here = () => `${publicPlotPath(slug)}${encodeQuery(query())}`;
	const [query, setQuery] = createSignal(initial);
	if (here() !== `${location.pathname}${location.search}`) {
		history.replaceState(history.state, "", here());
	}

	const [title, setTitle] = createSignal(slug);
	const [embeddable, setEmbeddable] = createSignal(false);
	const [view, setView] = createSignal<"plot" | "locked" | "checking">("plot");
	const readProject = (asReader: boolean): Promise<boolean> =>
		get<JsonProject>(project).then(
			(read) => {
				rememberVersion(localStorage, slug, read.bmf_version === 1 ? 1 : 0);
				setTitle(read.name);
				setEmbeddable(read.visibility === "public");
				document.title = `${read.name} | Bencher`;
				setView("plot");
				if (asReader && drawable(query())) {
					ask(query());
				}
				return true;
			},
			(error: Refusal) => {
				const refused = Boolean(error.status && error.status < 500);
				if (refused && token && !asReader) {
					get = reads(apiUrl, token);
					setView("checking");
					return readProject(true);
				}
				setView(refused ? "locked" : "plot");
				return !refused;
			},
		);
	// A plot refused before the project answers waits for it: a project no one may read shows no error.
	const readable = readProject(false);

	// Each answer kept, so going back to a window draws at once.
	const answers = new Map<string, JsonConsolePerf>();
	const [answer, setAnswer] = createSignal<JsonConsolePerf>();
	const [busy, setBusy] = createSignal(false);
	const [refused, setRefused] = createSignal<string>();
	let asked = 0;
	const ask = (next: ExploreQuery) => {
		const key = requestKey(next);
		const known = answers.get(key);
		const turn = ++asked;
		setRefused(undefined);
		if (known) {
			setAnswer(known);
			setBusy(false);
			return;
		}
		setBusy(true);
		get<JsonConsolePerf>(
			`/v0/console/projects/${encodeURIComponent(slug)}/perf${perfSearch(next, Date.now())}`,
		).then(
			(perf) => {
				answers.set(key, perf);
				if (turn === asked) {
					setAnswer(perf);
					setBusy(false);
				}
			},
			async (error: Refusal) => {
				const refusedQuery = Boolean(error.status && error.status < 500);
				if (refusedQuery && !(await readable)) {
					return;
				}
				if (turn === asked) {
					setRefused(refusalOf(error.status) ?? "The plot did not load");
					setBusy(false);
				}
			},
		);
	};

	const [plot, setPlot] = createSignal<PlotView>();
	if (drawable(initial)) {
		import("../plot/Plot")
			.then(({ default: loaded }) => setPlot(() => loaded))
			.catch(() => {});
	}

	const [copied, setCopied] = createSignal<Copied>();
	const edit = (next: ExploreQuery) => {
		setCopied(undefined);
		setQuery(next);
		history.replaceState(history.state, "", here());
	};

	handOff(mount, () =>
		render(() => {
			createEffect(
				on(
					() => requestKey(query()),
					() => {
						if (drawable(query())) {
							ask(query());
						}
					},
				),
			);
			return (
				<Public
					slug={slug}
					name={title()}
					query={query()}
					signIn={signInHref(here())}
					view={view()}
					answer={answer()}
					busy={busy()}
					refused={refused()}
					plot={plot()}
					embeddable={embeddable()}
					copied={copied()}
					onQuery={edit}
					onRetry={() => ask(query())}
					onCopy={(what) =>
						navigator.clipboard
							.writeText(
								what === "link"
									? location.href
									: embedCode({
											origin: location.origin,
											slug,
											query: query(),
											name: title(),
										}),
							)
							.then(() => setCopied(what))
							.catch(() => {})
					}
				/>
			);
		}, mount),
	);
};

const mount = document.getElementById("public");
if (mount) {
	boot(mount);
}
