import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import ButtonLink from "@bencherdev/ui/ButtonLink";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import ThemeToggle from "@bencherdev/ui/ThemeToggle";
import {
	For,
	Index,
	type JSX,
	Match,
	Show,
	Switch,
	createMemo,
	createSignal,
} from "solid-js";
import type { JsonConsolePerf } from "../../types/bencher";
import Controls from "../explore/Controls";
import { plotData } from "../explore/data";
import { measuresLayout } from "../explore/layout";
import { parameterRows } from "../explore/parameters";
import PlotFrame from "../explore/PlotFrame";
import { drawable } from "../explore/request";
import ShareIcon from "../explore/ShareIcon";
import { chipSummary } from "../explore/summary";
import { projectPath } from "../paths";
import { lineVisible, settleVisibility } from "../query/line";
import { type ExploreQuery, encodeQuery } from "../query/query";
import Mark from "../shell/Mark";
import { THEME_KEY } from "../theme";
import { publicPlotPath } from "./query";

export type PlotView = typeof import("../plot/Plot").default;

export interface PublicProps {
	slug: string;
	/** The project's name, or its slug until the project is read. */
	name: string;
	query: ExploreQuery;
	/** Sign in, carrying this page as the place to return to. */
	signIn: string;
	/**
	 * The plot; a project this reader cannot read; or, while a stored reader's
	 * access is checked, both: the page's head shows the one for its reader.
	 */
	view: "plot" | "locked" | "checking";
	answer?: JsonConsolePerf | undefined;
	/** A newer answer is on its way; the one drawn stays. */
	busy?: boolean | undefined;
	/** Why the last answer did not arrive. */
	refused?: string | undefined;
	plot?: PlotView | undefined;
	shared?: boolean | undefined;
	onQuery?: (query: ExploreQuery) => void;
	onRetry?: () => void;
	onShare?: () => void;
}

const SUMMARY = [
	["branches", "branches"],
	["testbeds", "testbeds"],
	["benchmarks", "benchmarks"],
	["sets", "parameters"],
	["measures", "measures"],
	["metrics", "metrics"],
] as const;

/** A version 1 project's plot for anyone who can read it; the page's head marks a stored reader before the first paint, and `pu-in` and `pu-out` show what is theirs. */
const Public = (props: PublicProps) => (
	<>
		<header class="shell">
			<div class="bar">
				<a class="brand" href="/" aria-label="Bencher, home">
					<Mark />
					<span class="brand-name">Bencher</span>
				</a>
				<nav class="crumbs" aria-label="Breadcrumb">
					{/* Only a project read names itself; the link's slug may name nothing. */}
					<Show when={props.view === "plot"}>
						<a class="projlink" href={publicPlotPath(props.slug)}>
							{props.name}
						</a>
					</Show>
				</nav>
				<span class="spacer" />
				<a class="docs" href="/docs/">
					Docs
				</a>
				<ThemeToggle storageKey={THEME_KEY} class="theme-toggle" />
				<a class="avatar pu-in" href="/console">
					<span class="sr-only">Account</span>
				</a>
				<span class="pu-out pu-auth">
					<ButtonLink size="sm" href={props.signIn}>
						Log in
					</ButtonLink>
					<ButtonLink
						class="pu-signup"
						size="sm"
						variant="primary"
						href="/auth/signup"
					>
						Sign up
					</ButtonLink>
				</span>
			</div>
		</header>
		<Switch>
			<Match when={props.view === "plot"}>
				<Plotted {...props} />
			</Match>
			<Match when={props.view === "locked"}>
				<Locked signIn={props.signIn} />
			</Match>
			<Match when={props.view === "checking"}>
				<div class="pu-out">
					<Locked signIn={props.signIn} />
				</div>
				<main class="page pu-in" aria-busy="true">
					<div class="pagehead">
						<Skeleton size="text" class="title-skel" />
					</div>
					<Skeleton size="card" />
				</main>
			</Match>
		</Switch>
	</>
);

export default Public;

const Plotted = (props: PublicProps) => {
	const [open, setOpen] = createSignal(false);
	const edit = (next: ExploreQuery) => props.onQuery?.(next);
	const data = createMemo(() => props.answer && plotData(props.answer));
	const drawn = createMemo(() => data()?.lines.map(({ id }) => id) ?? []);
	const settled = () => settleVisibility(props.query, drawn());
	const layout = () =>
		measuresLayout(props.query.measures.length, props.query.layout).layout;
	const reserve = createMemo(
		() => ({
			lines: props.query.only?.length ?? 1,
			measures: Math.max(1, props.query.measures.length),
		}),
		undefined,
		{ equals: (a, b) => a.lines === b.lines && a.measures === b.measures },
	);
	return (
		<main class="page pu">
			<div class="sethead">
				<div class="ph-title">
					<div class="crumb-line">Public plot</div>
					<Heading level={1} size="xl">
						{props.name}
					</Heading>
					<p class="ph-sub pu-out">
						Anyone can read this plot, up to 3 months back. Reports, alerts, and
						thresholds are for project members.
					</p>
					<p class="ph-sub pu-in">
						The public plot, as anyone sees it. The console has this project's
						reports, alerts, and thresholds.
					</p>
				</div>
				<div class="ph-actions pu-in">
					<ButtonLink
						variant="primary"
						href={`${projectPath(props.slug, "explore")}${encodeQuery(props.query)}`}
					>
						Open in console
					</ButtonLink>
				</div>
			</div>
			<section class="ex-qp" aria-label="Query, read only">
				<div class="pu-qhead">
					<span class="eyebrow">Query</span>
					{/* The names arrive with the plot, and a pill that grew would push the rest along. */}
					<span class="pu-sum">
						<Show
							when={!open()}
							fallback={
								<span class="muted sm pu-ro">
									read only: the six boxes behind this plot
								</span>
							}
						>
							<Show
								when={
									!drawable(props.query) ||
									props.answer !== undefined ||
									props.refused !== undefined
								}
								fallback={<Skeleton size="text" class="pu-sumskel" />}
							>
								<For each={SUMMARY}>
									{([dimension, label]) => (
										<span class="pu-pill">
											<span class="pu-k">{label}</span>
											<span class="ex-ellip">
												{chipSummary(
													props.query,
													dimension,
													props.answer,
													false,
												)}
											</span>
										</span>
									)}
								</For>
							</Show>
						</Show>
					</span>
					<Button
						size="sm"
						aria-expanded={open()}
						onClick={() => setOpen(!open())}
					>
						<span>{open() ? "Hide query" : "Show query"}</span>
						<Icon name="chevron-down" />
					</Button>
				</div>
				<Show when={open()}>
					<Boxes query={props.query} answer={props.answer} />
				</Show>
				<Controls
					query={props.query}
					total={undefined}
					now={Date.now}
					onQuery={edit}
					reach={
						props.answer?.window.clamped ? (
							<>
								cut to its last 3 months
								<span class="pu-out">
									. <a href={props.signIn}>Log in</a> to see it all
								</span>
							</>
						) : (
							"up to 3 months back"
						)
					}
				>
					<span class="spacer" />
					<Button size="sm" onClick={() => props.onShare?.()}>
						<ShareIcon />
						{props.shared ? "Link copied" : "Share"}
					</Button>
				</Controls>
			</section>
			<Show
				when={drawable(props.query)}
				fallback={
					<p class="lede">
						This link names no lines to draw. A project member picks them in
						the console and shares the link.
					</p>
				}
			>
				<section class="ex-plot" aria-label="Plot" aria-busy={props.busy}>
					<Show when={props.refused}>
						{(text) => (
							<div class="ex-failed">
								<Banner status="error" role="alert" class="load-error">
									<span class="grow">{text()}</span>
									<Button
										size="lg"
										disabled={props.busy}
										onClick={() => props.onRetry?.()}
									>
										Retry
									</Button>
								</Banner>
							</div>
						)}
					</Show>
					<Show
						when={(props.answer !== undefined || !props.refused) && props.plot}
						fallback={
							<PlotFrame
								reserve={reserve()}
								layout={layout()}
								loading={!props.refused}
							/>
						}
					>
						{(View) => {
							const Loaded = View();
							return (
								<Loaded
									data={data()}
									hidden={drawn().filter((id) => !lineVisible(props.query, id))}
									focused={props.query.focus ?? null}
									onHiddenChange={(hide) => edit({ ...settled(), hide })}
									onFocusChange={(focus) => {
										const { focus: _focus, ...rest } = settled();
										edit(focus === null ? rest : { ...rest, focus });
									}}
									xAxis={props.query.xAxis}
									scale={props.query.yScale}
									layout={layout()}
									reserve={reserve()}
								/>
							);
						}}
					</Show>
				</section>
			</Show>
		</main>
	);
};

const NAMED = [
	["branches", "Branches"],
	["testbeds", "Testbeds"],
	["benchmarks", "Benchmarks"],
] as const;

/** The six boxes behind the plot, read only, named from what the plot drew. */
const Boxes = (props: {
	query: ExploreQuery;
	answer: JsonConsolePerf | undefined;
}) => {
	const named = (
		dimension: "branches" | "testbeds" | "benchmarks" | "measures",
	): Value[] => {
		const uuids =
			dimension === "branches" || dimension === "testbeds"
				? props.query[dimension].map(({ uuid }) => uuid)
				: props.query[dimension];
		const table: { uuid: string; name: string; units?: string }[] =
			props.answer?.[dimension] ?? [];
		return uuids.map((uuid) => {
			const entry = table.find((row) => row.uuid === uuid);
			return {
				label: entry?.name ?? "not drawn in this window",
				detail: entry?.units,
			};
		});
	};
	const rows = createMemo(() =>
		parameterRows(
			props.query.sets,
			(props.answer?.variants ?? []).map(({ parameters }) => parameters),
		),
	);
	return (
		<div class="ex-editor">
			<For each={NAMED}>
				{([dimension, title]) => (
					<Box title={title}>
						<Values values={named(dimension)} />
					</Box>
				)}
			</For>
			<Box title="Parameters">
				<Index
					each={props.query.sets}
					fallback={<p class="ex-all">every variant</p>}
				>
					{(set, index) => (
						<div class="ex-set">
							<span class="ex-tags">
								<For
									each={Object.entries(set()).map(
										([key, value]) => `${key}=${String(value)}`,
									)}
								>
									{(tag) => <span class="ex-tag">{tag}</span>}
								</For>
							</span>
							<span class="ex-pc">
								{plural(rows()[index]?.variants ?? 0, "variant", "variants")}
							</span>
						</div>
					)}
				</Index>
			</Box>
			<Box title="Measures">
				<Values values={named("measures")} />
			</Box>
			<Box title="Metrics">
				<Show
					when={props.query.metrics.length > 0}
					fallback={<p class="ex-all">every metric</p>}
				>
					<Values values={props.query.metrics.map((label) => ({ label }))} />
				</Show>
			</Box>
		</div>
	);
};

interface Value {
	label: string;
	detail?: string | undefined;
}

const Box = (props: { title: string; children: JSX.Element }) => (
	<fieldset class="ex-box" aria-label={props.title}>
		<div class="ex-eh">
			<span>{props.title}</span>
		</div>
		{props.children}
	</fieldset>
);

const Values = (props: { values: Value[] }) => (
	<For each={props.values}>
		{(value) => (
			<div class="ex-val">
				<span class="ex-ellip">{value.label}</span>
				<Show when={value.detail}>
					<span class="ex-k">{value.detail}</span>
				</Show>
			</div>
		)}
	</For>
);

const plural = (count: number, one: string, many: string) =>
	`${count} ${count === 1 ? one : many}`;

/** The page a reader sees for a project they cannot read, which may not exist; a signed in reader is not asked to log in. */
const Locked = (props: { signIn: string }) => (
	<main class="page pu-locked">
		<section class="pu-lockcard pu-out" aria-labelledby="pu-locked">
			<span class="pu-lock" aria-hidden="true">
				<Icon name="read-only" />
			</span>
			<Heading level={1} size="xl" id="pu-locked">
				Log in to see this page
			</Heading>
			<p class="lede">
				This page is private, or it does not exist. Log in with an account that
				belongs to its project.
			</p>
			<div class="btnrow">
				<ButtonLink variant="primary" href={props.signIn}>
					Log in
				</ButtonLink>
				<ButtonLink href="/auth/signup">Sign up</ButtonLink>
			</div>
		</section>
		<section class="pu-lockcard pu-in" aria-labelledby="pu-private">
			<span class="pu-lock" aria-hidden="true">
				<Icon name="read-only" />
			</span>
			<Heading level={1} size="xl" id="pu-private">
				This project is private, or it does not exist
			</Heading>
			<p class="lede">
				Your account cannot see it. Ask a member of its project for access.
			</p>
		</section>
	</main>
);
