import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import type { JsonConsolePerf, JsonProject } from "../../types/bencher";
import { drawable, perfSearch, refusalOf } from "../explore/request";
import { handOff } from "../handoff";
import { type Refusal, reads } from "../public/read";
import { decodeQuery } from "../query/query";
import Embed, { type PlotView } from "./Embed";

const boot = (mount: HTMLElement) => {
	const { apiUrl = "", slug = "" } = mount.dataset;
	// Another site's frame has no reader: the embed shows what anyone sees.
	const get = reads(apiUrl);
	const path = `/v0/projects/${encodeURIComponent(slug)}`;
	const [query, setQuery] = createSignal(decodeQuery(location.search));
	const [name, setName] = createSignal<string>();
	const [nameless, setNameless] = createSignal(false);
	const [view, setView] = createSignal<"plot" | "locked">("plot");
	const [answer, setAnswer] = createSignal<JsonConsolePerf>();
	const [refused, setRefused] = createSignal<string>();
	const [plot, setPlot] = createSignal<PlotView>();

	get<JsonProject>(path).then(
		(project) => {
			setName(project.name);
			document.title = `${project.name} | Bencher`;
		},
		(error: Refusal) => {
			if (error.status && error.status < 500) {
				setView("locked");
			} else {
				setNameless(true);
			}
		},
	);
	if (drawable(query())) {
		import("../plot/Plot")
			.then(({ default: loaded }) => setPlot(() => loaded))
			.catch(() => {});
		get<JsonConsolePerf>(
			`/v0/console/projects/${encodeURIComponent(slug)}/perf${perfSearch(query(), Date.now())}`,
		).then(setAnswer, (error: Refusal) =>
			setRefused(refusalOf(error.status) ?? "The plot did not load"),
		);
	}

	handOff(mount, () =>
		render(
			() => (
				<Embed
					slug={slug}
					name={name()}
					nameless={nameless()}
					query={query()}
					view={view()}
					answer={answer()}
					refused={refused()}
					plot={plot()}
					onQuery={setQuery}
				/>
			),
			mount,
		),
	);
};

const mount = document.getElementById("embed");
if (mount) {
	boot(mount);
}
