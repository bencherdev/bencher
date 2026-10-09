import Card from "@bencherdev/ui/Card";
import { For, Show } from "solid-js";
import type {
	JsonConsoleThreshold,
	JsonConsoleThresholdModel,
} from "../../types/bencher";
import Code from "../report/Code";
import {
	archivedBy,
	dayText,
	filterSets,
	historySpan,
	modelRows,
	modelText,
} from "./model";
import { VALUE } from "./rows";
import Sets from "./Sets";
import { declarationSnippet } from "./snippet";

// Bencher Cloud's API, which `bencher run` reports to unless told otherwise.
const CLOUD = "https://api.bencher.dev";
const CHANGE = "th-change";

/** What the threshold applies to, its model, and every model it has had. */
export const Cards = (props: {
	threshold: JsonConsoleThreshold;
	now: number;
}) => {
	const archived = () => archivedBy(props.threshold);
	return (
		<div class="th-cards">
			<Card
				as="section"
				variant="soft"
				class="th-card"
				aria-labelledby="th-applies"
			>
				<h2 class="seclabel" id="th-applies">
					Applies to
				</h2>
				<dl>
					<div class="kv">
						<dt>branch</dt>
						<dd>
							{props.threshold.branch.name}
							<Show when={props.threshold.branch.start_point}>
								{(from) => <span class="th-quiet">, from {from()}</span>}
							</Show>
						</dd>
					</div>
					<div class="kv">
						<dt>testbed</dt>
						<dd>{props.threshold.testbed.name}</dd>
					</div>
					<div class="kv">
						<dt>measure</dt>
						<dd>
							{props.threshold.measure.name}
							<span class="th-quiet">, {props.threshold.measure.units}</span>
						</dd>
					</div>
					<div class="kv">
						<dt>metric</dt>
						<dd class="th-mono">{props.threshold.metric ?? VALUE}</dd>
					</div>
					<div class="kv">
						<dt>parameters</dt>
						<dd>
							<Show
								when={props.threshold.parameters?.length}
								fallback={
									<>
										every variant <span class="th-quiet">(no filter)</span>
									</>
								}
							>
								<Sets sets={filterSets(props.threshold.parameters)} />
							</Show>
						</dd>
					</div>
				</dl>
				<Show when={archived()}>
					{(by) => (
						<p class="th-note">
							Archived {dayText(by().archived, props.now)} with {by().kind}{" "}
							{by().name}
						</p>
					)}
				</Show>
			</Card>
			<Card
				as="section"
				variant="soft"
				class="th-card"
				aria-labelledby="th-model"
			>
				<h2 class="seclabel" id="th-model">
					Model
				</h2>
				<Show
					when={props.threshold.model}
					fallback={
						<p class="th-note">
							No model: it checks nothing until a run declares one.
						</p>
					}
				>
					{(model) => (
						<>
							<dl>
								<div class="kv">
									<dt>test</dt>
									<dd class="th-mono">{model().test}</dd>
								</div>
								<For each={modelRows(model())}>
									{([field, value]) => (
										<div class="kv">
											<dt>{field}</dt>
											<dd classList={{ "th-quiet": value === undefined }}>
												{value ?? "none"}
											</dd>
										</div>
									)}
								</For>
							</dl>
							<p class="th-note">
								Set {dayText(model().created, props.now)}.{" "}
								<a href={`#${CHANGE}`}>Change it from the run</a>.
							</p>
						</>
					)}
				</Show>
			</Card>
			<Card
				as="section"
				variant="soft"
				class="th-card"
				aria-labelledby="th-history"
			>
				<h2 class="seclabel" id="th-history">
					Model history
				</h2>
				<ol class="th-history">
					<For each={props.threshold.models}>
						{(model) => <HistoryRow model={model} now={props.now} />}
					</For>
				</ol>
			</Card>
		</div>
	);
};

const HistoryRow = (props: {
	model: JsonConsoleThresholdModel;
	now: number;
}) => {
	const current = () => props.model.replaced === undefined;
	return (
		<li classList={{ "th-replaced": !current() }}>
			<span class="th-pill" classList={{ "th-pill-on": current() }}>
				{current() ? "current" : "replaced"}
			</span>{" "}
			<span class="th-mono">{modelText(props.model)}</span>{" "}
			<span class="th-quiet">{historySpan(props.model, props.now)}</span>
		</li>
	);
};

/** How to change the threshold: the run that declares it as it is. */
export const Change = (props: {
	slug: string;
	threshold: JsonConsoleThreshold;
	host: string;
}) => {
	const where = () =>
		`${props.threshold.branch.name} · ${props.threshold.testbed.name}`;
	const snippet = () => {
		const model = props.threshold.model;
		return (
			model &&
			declarationSnippet(
				{
					project: props.slug,
					branch: props.threshold.branch.name,
					testbed: props.threshold.testbed.name,
					host: props.host === CLOUD ? undefined : props.host,
				},
				{
					measure: props.threshold.measure.slug,
					metric: props.threshold.metric ?? VALUE,
					parameters: props.threshold.parameters,
					model,
				},
			)
		);
	};
	return (
		<Card
			as="section"
			variant="soft"
			class="th-card th-change"
			aria-labelledby={CHANGE}
		>
			<h2 class="seclabel" id={CHANGE}>
				Change this threshold
			</h2>
			<div class="th-change-body">
				<div class="th-change-text">
					<p>
						The console never edits a threshold; the run that declares it does.
						The next run on {where()} that declares a different model replaces
						this one and adds a row to its history.
					</p>
					<p>
						To stop it checking, leave it out of the run and pass{" "}
						<span class="th-mono">--thresholds-reset</span>: the threshold keeps
						its history and loses its model.
					</p>
					<a
						class="lnk"
						href="https://bencher.dev/docs/explanation/thresholds/"
					>
						How thresholds work
					</a>
				</div>
				<Show
					when={snippet()}
					fallback={
						<p class="th-note">
							It has no model to declare again; a run that declares one for{" "}
							{props.threshold.measure.name} on {where()} gives it one.
						</p>
					}
				>
					{(declared) => (
						<fieldset class="th-declared">
							<legend class="sr-only">
								The run that declares this threshold
							</legend>
							<Show when={declared().kind === "payload"}>
								<p class="th-note">
									Each run flag carries one parameters set, so a filter of{" "}
									{props.threshold.parameters?.length} sets is declared in a
									report's <span class="th-mono">thresholds.models</span>, sent
									to the API:
								</p>
							</Show>
							<Code
								name={
									declared().kind === "flags"
										? "the bencher run command"
										: "the report's thresholds"
								}
								text={declared().text}
								code={declared().code}
							/>
						</fieldset>
					)}
				</Show>
			</div>
		</Card>
	);
};
