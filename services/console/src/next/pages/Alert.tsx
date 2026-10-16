import { useParams } from "@solidjs/router";
import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import {
	For,
	type JSX,
	Match,
	Show,
	Switch,
	createMemo,
	createSignal,
	onCleanup,
	onMount,
} from "solid-js";
import type { JsonConsoleAlertDetail } from "../../types/bencher";
import { type AlertFacts, alertFacts, alertQuery } from "../alert/detail";
import { useAlertChange } from "../alerts/change";
import { ApiError } from "../api";
import { plotData } from "../explore/data";
import { projectPath, reportPath } from "../paths";
import Plot from "../plot/Plot";
import { formatDate, formatWhen } from "../plot/format";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";

const NOTE = "4w, ending at its report";
// The Alerts list's words: its code would split from that page's chunk for this one line.
const SILENCED = "Silenced: branch head replaced";

// A UUID, hyphenated or simple, as the API parses one; it refuses anything else before looking.
const UUID =
	/^(?:[0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/i;

/** One alert: the line it was raised on, its status and action, its plot, and its facts. */
const Alert = () => {
	const { api, slug } = useProject();
	const params = useParams<{ alert: string }>();
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	const valid = () => UUID.test(params.alert);
	const query = useQueryResult(() => ({
		...alertQuery(api, slug(), params.alert),
		enabled: valid(),
	}));
	const detail = () => query().data;
	const facts = createMemo(() => {
		const current = detail();
		return current && alertFacts(current);
	});
	const changes = useAlertChange();
	const status = () => {
		const current = detail();
		return current && (changes.status(current.uuid) ?? current.status);
	};
	// The plot keeps its drawing while only the alert's own facts change.
	const perf = createMemo(() => detail()?.perf);
	const drawn = createMemo(() => {
		const current = perf();
		return current && plotData(current);
	});

	// Explore's query codec loads once the page has drawn and settled, not with it.
	const [explorer, setExplorer] =
		createSignal<typeof import("../alert/explore")["exploreSearchOf"]>();
	// At the browser's next idle moment, at most a second away, as a list's row links arrive; their scheduler is in their chunk.
	onMount(() => {
		const load = () => {
			performance.mark("console:idle-load");
			import("../alert/explore")
				.then(({ exploreSearchOf }) => setExplorer(() => exploreSearchOf))
				.catch(() => {});
		};
		if ("requestIdleCallback" in window) {
			const id = requestIdleCallback(load, { timeout: 1_000 });
			onCleanup(() => cancelIdleCallback(id));
		} else {
			const id = setTimeout(load, 1_000);
			onCleanup(() => clearTimeout(id));
		}
	});
	const explore = () => {
		const current = detail();
		const search = current && explorer()?.(current);
		return search === undefined
			? undefined
			: `${projectPath(slug(), "explore")}${search}`;
	};

	const failure = () => {
		const { data, error } = query();
		return data || !error
			? undefined
			: error instanceof ApiError
				? error.kind
				: "network";
	};
	const missing = () => !valid() || failure() === "not_found";
	const readOnly = () => bootstrap().data?.permissions.edit === false;

	return (
		<Show when={!missing()} fallback={<NotFound slug={slug()} />}>
			<Switch>
				<Match
					when={bootstrap().data && detail() && facts() ? detail() : undefined}
				>
					{(current) => (
						<Page
							slug={slug()}
							detail={current()}
							facts={facts() as AlertFacts}
							status={status() ?? current().status}
							justNow={changes.status(current().uuid) !== undefined}
							readOnly={readOnly()}
							refused={changes.refused(current().uuid)}
							onChange={(next) => changes.change(current().uuid, next)}
							plot={drawn()}
							explore={explore()}
						/>
					)}
				</Match>
				<Match when={failure()}>
					<main class="page">
						<div class="pagehead">
							<div class="ph-title">
								<Crumb slug={slug()} />
								<Heading level={1} size="xl">
									Alert
								</Heading>
							</div>
						</div>
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">This alert did not load</span>
							<Button size="sm" onClick={() => query().refetch()}>
								Retry
							</Button>
						</Banner>
					</main>
				</Match>
				<Match when={true}>
					<Loading slug={slug()} />
				</Match>
			</Switch>
		</Show>
	);
};

export default Alert;

const Page = (props: {
	slug: string;
	detail: JsonConsoleAlertDetail;
	facts: AlertFacts;
	status: "active" | "dismissed" | "silenced";
	/** Changed on this page, while the reader stays. */
	justNow: boolean;
	readOnly: boolean;
	refused: boolean;
	onChange: (status: "active" | "dismissed") => void;
	plot: ReturnType<typeof plotData> | undefined;
	explore: string | undefined;
}) => {
	const [hidden, setHidden] = createSignal<string[]>([]);
	const [focused, setFocused] = createSignal<string | null>();
	const action = () =>
		props.readOnly || props.status === "silenced"
			? undefined
			: props.status === "active"
				? "Dismiss"
				: "Reactivate";
	const when = () => formatWhen(props.detail.report.start_time);
	const hash = () => props.detail.report.hash?.slice(0, 7);
	return (
		<main class="page">
			<div class="pagehead rp-head">
				<div class="ph-title">
					<Crumb slug={props.slug} readOnly={props.readOnly} />
					<div class="ap-titlerow">
						<Heading level={1} size="xl">
							<span>{props.facts.benchmark}</span>
							<For each={props.facts.tags}>
								{(tag) => (
									<>
										{" "}
										<span class="lr-ptag ap-tag">{tag}</span>
									</>
								)}
							</For>
							<Show when={props.facts.metric}>
								{(metric) => (
									<>
										{" "}
										<span class="lr-tag ap-tag">{metric()}</span>
									</>
								)}
							</Show>
						</Heading>
						<span
							class="rp-alerts"
							classList={{ off: props.status !== "active" }}
						>
							<svg class="lr-alertdot" viewBox="0 0 8 8" aria-hidden="true">
								<circle cx="4" cy="4" r="4" />
							</svg>
							{props.status === "active" ? (
								"Active"
							) : props.status === "silenced" ? (
								SILENCED
							) : props.justNow ? (
								"Dismissed just now"
							) : (
								<>
									Dismissed{" "}
									<time
										datetime={new Date(props.detail.modified).toISOString()}
									>
										{formatDate(props.detail.modified)}
									</time>
								</>
							)}
						</span>
					</div>
				</div>
				<Show when={action()}>
					{(label) => (
						<div class="ph-actions rp-actions">
							<Button
								variant="primary"
								size="sm"
								class="ap-act"
								aria-label={`${label()} the alert on ${props.facts.name}`}
								onClick={() =>
									props.onChange(
										props.status === "active" ? "dismissed" : "active",
									)
								}
							>
								{label()}
							</Button>
						</div>
					)}
				</Show>
				<p class="ph-sub">
					{props.facts.measure}
					<span class="dotsep" aria-hidden="true">
						·
					</span>
					{props.facts.branch}
					<span class="dotsep" aria-hidden="true">
						·
					</span>
					{props.facts.testbed}
				</p>
			</div>
			<Show when={props.refused}>
				<Banner status="error" role="alert" class="load-error">
					<span class="grow">The alert did not change</span>
				</Banner>
			</Show>
			<Plot
				data={props.plot}
				hidden={hidden()}
				focused={focused()}
				onHiddenChange={setHidden}
				onFocusChange={setFocused}
				note={NOTE}
				label={`${props.facts.name}, 4 weeks ending at its report`}
				reportHref={(uuid) => reportPath(props.slug, uuid)}
				pin={
					props.detail.perf.lines[0]?.series.alerts.find(
						({ uuid }) => uuid === props.detail.uuid,
					)?.index
				}
				alerts={{
					status: (uuid) =>
						uuid === props.detail.uuid ? props.status : undefined,
				}}
			/>
			<section class="ap-facts" aria-label="The alert">
				<dl>
					<Fact term="Value" number>
						{props.facts.value}
					</Fact>
					<Show when={props.facts.limit}>
						{(limit) => (
							<Fact
								term={
									props.facts.side === "lower" ? "Lower limit" : "Upper limit"
								}
								number
							>
								{limit()}
							</Fact>
						)}
					</Show>
					<Show when={props.facts.baseline}>
						{(baseline) => (
							<Fact term="Baseline" number>
								{baseline()}
							</Fact>
						)}
					</Show>
					<Show when={props.facts.delta}>
						{(delta) => (
							<div class="ap-fact">
								<dt class="eyebrow">Delta</dt>
								<dd
									class="lr-num"
									classList={{
										"lr-worse": delta().tone === "worse",
										"lr-better": delta().tone === "better",
									}}
								>
									<Show when={delta().arrow}>
										<span aria-hidden="true">{delta().arrow} </span>
									</Show>
									{delta().text}
									<Show when={delta().tone}> {delta().tone}</Show>
								</dd>
							</div>
						)}
					</Show>
					<Fact term="Report">
						<a
							href={reportPath(props.slug, props.detail.report.uuid)}
							aria-label={`${[when(), hash()].filter(Boolean).join(" · ")}, open the report`}
						>
							{when()}
							<Show when={hash()}>
								{(shown) => (
									<>
										{" "}
										<span class="muted">· {shown()}</span>
									</>
								)}
							</Show>
						</a>
					</Fact>
					<Show when={props.facts.threshold}>
						{(threshold) => (
							<Fact term="Threshold">
								<a
									class="th-mono"
									href={`${projectPath(props.slug, "thresholds")}/${threshold().uuid}`}
									aria-label={`${threshold().text}, open the threshold`}
								>
									{threshold().text}
								</a>
							</Fact>
						)}
					</Show>
				</dl>
				<Show
					when={props.explore}
					fallback={
						<Button size="sm" class="ap-go" disabled>
							Open in Explore
						</Button>
					}
				>
					{(href) => (
						<a
							class="ui-button ap-go"
							data-variant="secondary"
							data-size="sm"
							href={href()}
							aria-label={`Open in Explore: ${props.facts.name}`}
						>
							Open in Explore
						</a>
					)}
				</Show>
			</section>
		</main>
	);
};

const Fact = (props: {
	term: string;
	number?: boolean;
	children: JSX.Element;
}) => (
	<div class="ap-fact">
		<dt class="eyebrow">{props.term}</dt>
		<dd classList={{ "lr-num": props.number === true }}>{props.children}</dd>
	</div>
);

/** The way back to Alerts, and whether the reader can only read, where its arrival takes no width from the title. */
const Crumb = (props: { slug: string; readOnly?: boolean }) => (
	<div class="crumb-line">
		<a href={projectPath(props.slug, "alerts")}>Alerts</a>
		<span aria-hidden="true">/</span>
		<Show when={props.readOnly}>
			<span class="rp-ro">
				<Icon name="read-only" />
				Read only
			</span>
		</Show>
	</div>
);

/** The page's frame before the alert and the reader's permissions arrive; the page replaces it whole, so nothing in it moves. */
const Loading = (props: { slug: string }) => (
	<main class="page" aria-busy="true">
		<div class="pagehead rp-head">
			<div class="ph-title">
				<Crumb slug={props.slug} />
				<div class="ap-titlerow">
					<Skeleton size="text" class="ap-title-skel" />
					<span class="sr-only">Loading the alert</span>
				</div>
			</div>
			<p class="ph-sub">
				<Skeleton size="text" class="al-sub-skel" />
			</p>
		</div>
		<Skeleton size="card" class="ap-plot-skel" />
		<Skeleton size="card" class="ap-facts-skel" />
	</main>
);

const NotFound = (props: { slug: string }) => (
	<main class="page">
		<div class="pagehead">
			<div class="ph-title">
				<Crumb slug={props.slug} />
				<Heading level={1} size="xl">
					Alert not found
				</Heading>
			</div>
		</div>
	</main>
);
