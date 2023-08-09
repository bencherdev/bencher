import { Show, createMemo, createResource } from "solid-js";
import { useNavigate } from "../../../util/url";
import { authUser } from "../../../util/auth";
import { BENCHER_API_URL } from "../../../util/ext";
import { httpGet } from "../../../util/http";
import type { JsonAlertStats } from "../../../types/bencher";
import type { Params } from "astro";

interface Props {
	params: Params;
}

enum Section {
	PERF = "perf",
	REPORTS = "reports",
	METRIC_KINDS = "metric-kinds",
	BRANCHES = "branches",
	TESTBEDS = "testbeds",
	BENCHMARKS = "benchmarks",
	THRESHOLDS = "thresholds",
	ALERTS = "alerts",
	SETTINGS = "settings",
}

const ConsoleMenu = (props: Props) => {
	const navigate = useNavigate();
	const user = authUser();

	const getAlerts = async (params: Params): Promise<JsonAlertStats> => {
		const DEFAULT_ALERT_STATS = {
			active: 0,
		};
		if (!params.project) {
			return DEFAULT_ALERT_STATS;
		}
		const url = `${BENCHER_API_URL()}/v0/projects/${
			params.project
		}/stats/alerts`;
		return await httpGet(url, authUser()?.token)
			.then((resp) => resp.data)
			.catch((error) => {
				console.error(error);
				return DEFAULT_ALERT_STATS;
			});
	};
	const [alert_stats] = createResource(props.params, getAlerts);
	const active_alerts = createMemo(() => alert_stats()?.active);

	const path = (section: Section) =>
		`/console/users/${user?.user?.slug}/${section}`;

	return (
		<aside class="menu is-sticky">
			<div class="menu-label">
				<button
					class="button is-outlined is-fullwidth"
					title="View Project Perf"
					onClick={(e) => {
						e.preventDefault();
						navigate(path(Section.PERF));
					}}
				>
					<span class="icon">
						<i class="fas fa-chart-line" aria-hidden="true" />
					</span>
				</button>
			</div>
			<p class="menu-label">Project</p>
			<ul class="menu-list">
				<li>
					<a href={path(Section.REPORTS)}>Reports</a>
				</li>
				<li>
					<a href={path(Section.METRIC_KINDS)}>Metric Kinds</a>
				</li>
				<li>
					<a href={path(Section.BRANCHES)}>Branches</a>
				</li>
				<li>
					<a href={path(Section.TESTBEDS)}>Testbeds</a>
				</li>
				<li>
					<a href={path(Section.BENCHMARKS)}>Benchmarks</a>
				</li>
				<li>
					<a href={path(Section.THRESHOLDS)}>Thresholds</a>
				</li>
				<li>
					<a href={path(Section.ALERTS)}>
						<nav class="level">
							<div class="level-left">
								<div class="level-item">Alerts</div>
								<Show when={active_alerts()} fallback={<></>}>
									<div class="level-item">
										<button class="button is-primary is-small is-rounded">
											{active_alerts()}
										</button>
									</div>
								</Show>
							</div>
						</nav>
					</a>
				</li>
				<li>
					<a href={path(Section.SETTINGS)}>Settings</a>
				</li>
			</ul>
		</aside>
	);
};

export default ConsoleMenu;
