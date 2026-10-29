import Truncate from "@bencherdev/ui/Truncate";
import { Show, createMemo, createSignal } from "solid-js";
import type { JsonConsoleAlerts } from "../../types/bencher";
import { type AlertLine, alertLinesOf } from "../alerts/lines";
import type { Settable } from "../alerts/status";
import { alertPath, projectPath, reportPath } from "../paths";
import Dot from "../dot";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import { type ReportLine, plotOf } from "../report/lines";
import { FOLD } from "../report/fold";
import LineTable from "../report/LineTable";
import { onMoved } from "./Neighbor";

/** The API's history window for each alert, four weeks before its report. */
const HISTORY_DAYS = 28;

/** A run's alerts as its reports' pages draw their lines, each naming its report in the run. */
const RunAlerts = (props: {
	slug: string;
	label: string;
	alerting: JsonConsoleAlerts;
	/** The alerts whose lines show; the rest wait for the reader. */
	shown: ReadonlySet<string>;
	onReportIntent: (uuid: string) => void;
}) => {
	const { api } = useProject();
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, props.slug));
	const canChange = () => bootstrap().data?.permissions.edit === true;
	const lines = createMemo(() =>
		alertLinesOf(props.alerting).filter((line) => props.shown.has(line.key)),
	);
	const metrics = createMemo(
		() => new Set(lines().map((line) => line.metric)).size > 1,
	);
	const [expanded, setExpanded] = createSignal<ReadonlySet<string>>(new Set());
	// The statuses its row plots set, which outlive a row folded and opened again.
	const kept = new Map<string, Settable>();
	// Explore's query codec loads once the lines want their links, not with the page.
	const [explorer, setExplorer] =
		createSignal<typeof import("../report/explore")["exploreSearchOf"]>();
	const loadExplorer = () => {
		if (!explorer()) {
			import("../report/explore")
				.then(({ exploreSearchOf }) => setExplorer(() => exploreSearchOf))
				.catch(() => {});
		}
	};
	/** Each row's line in Explore, ending at its own report. */
	const explore = () => {
		const searchOf = explorer();
		return searchOf
			? (shown: readonly ReportLine[]) => {
					const { report } = shown[0] as AlertLine;
					return `${projectPath(props.slug, "explore")}${searchOf(
						shown,
						{ uuid: report.uuid, end: report.end },
						HISTORY_DAYS,
					)}`;
				}
			: undefined;
	};
	return (
		<div class="lr-alerts">
			<LineTable
				label={props.label}
				slots={lines()}
				rows={lines().length}
				fold={FOLD.run}
				metrics={metrics()}
				history="History, 4w"
				expanded={expanded()}
				onExpand={(line, open) => {
					const next = new Set(expanded());
					if (open) {
						next.add(line.key);
					} else {
						next.delete(line.key);
					}
					setExpanded(next);
				}}
				beside={{
					label: "Report",
					column: "th-c-report",
					cell: (line, narrow) => {
						const { report, testbed } = line as AlertLine;
						const name = `#${report.order} · ${testbed.name}`;
						const intent = () => props.onReportIntent(report.uuid);
						// A phone row names its report by place alone, as text, so its measure keeps its room.
						return (
							<Show
								when={!narrow()}
								fallback={
									<>
										#{report.order}
										<span class="sr-only"> · {testbed.name}</span>
									</>
								}
							>
								<span class="th-report">
									<a
										class="ui-parts"
										href={reportPath(props.slug, report.uuid)}
										aria-label={`${name}, open the report`}
										onPointerMove={onMoved(intent)}
										onFocus={intent}
										onTouchStart={intent}
									>
										<span data-fixed>#{report.order}</span>
										<Dot />
										<Truncate text={testbed.name} tail />
									</a>
								</span>
							</Show>
						);
					},
				}}
				dimmed={(line) => (line as AlertLine).status !== "active"}
				explore={explore()}
				onExploreIntent={loadExplorer}
				href={(line) => alertPath(props.slug, line.key)}
				plot={(line) => ({
					data: plotOf(line, line.branch.name, line.testbed.name),
					note: "4w, ending at its report",
					reportHref: (uuid) => reportPath(props.slug, uuid),
					kept,
					canChange,
				})}
			/>
		</div>
	);
};

export default RunAlerts;
