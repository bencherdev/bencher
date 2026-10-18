import { createEffect, createSignal, on } from "solid-js";
import { render } from "solid-js/web";
import type { JsonConsolePerf, JsonProject } from "../../types/bencher";
import { embedCode } from "../embed/code";
import { drawable, perfSearch, requestKey } from "../explore/request";
import type { Copied } from "../explore/Share";
import { handOff } from "../handoff";
import { rememberVersion } from "../memory";
import { type ExploreQuery, decodeQuery, encodeQuery } from "../query/query";
import { isCurrent, readReader, signInHref } from "../reader";
import Public, { type PlotView } from "./Public";
import { publicPlotPath } from "./query";
import { type Refusal, reads } from "./read";

const boot = (mount: HTMLElement) => {
	const { apiUrl = "", slug = "", name = slug, found } = mount.dataset;
	const reader = readReader(localStorage);
	const token =
		reader && isCurrent(reader, Date.now()) ? reader.token : undefined;
	// A project the page read without a sign in draws the same for everyone.
	const sent = found === "public" ? undefined : token;
	const get = reads(apiUrl, sent);
	const project = `/v0/projects/${encodeURIComponent(slug)}`;

	const initial = decodeQuery(location.search);
	const here = () => `${publicPlotPath(slug)}${encodeQuery(query())}`;
	const [query, setQuery] = createSignal(initial);
	if (here() !== `${location.pathname}${location.search}`) {
		history.replaceState(history.state, "", here());
	}
	if (found === "public") {
		rememberVersion(localStorage, slug, 1);
	}

	const [title, setTitle] = createSignal(name);
	const [embeddable, setEmbeddable] = createSignal(found === "public");
	// As the page painted it: a project the server could not read at all is
	// drawn as a plot until the reader's own read says otherwise.
	const [view, setView] = createSignal<"plot" | "locked" | "checking">(
		found !== "private" ? "plot" : token ? "checking" : "locked",
	);
	// Without a reader, the API already said it shows this project to no one.
	const blocked = found === "private" && !token;
	if (found !== "public" && !blocked) {
		get<JsonProject>(project).then(
			(read) => {
				rememberVersion(localStorage, slug, read.bmf_version === 1 ? 1 : 0);
				if (read.bmf_version !== 1) {
					location.replace(
						`${read.visibility === "public" ? `/perf/${slug}` : `/console/projects/${slug}/perf`}${location.search}`,
					);
					return;
				}
				setTitle(read.name);
				setEmbeddable(read.visibility === "public");
				document.title = `${read.name} | Bencher`;
				setView("plot");
			},
			(error: Refusal) => {
				setView(error.status && error.status < 500 ? "locked" : "plot");
			},
		);
	}

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
			(error: Refusal) => {
				if (turn === asked) {
					setRefused(
						error.status && error.status < 500
							? "The API refused this query"
							: "The plot did not load",
					);
					setBusy(false);
				}
			},
		);
	};

	const [plot, setPlot] = createSignal<PlotView>();
	if (drawable(initial) && !blocked) {
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
						if (drawable(query()) && !blocked) {
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
