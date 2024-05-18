import type { Params } from "astro";
import bencher_valid_init, { type InitOutput } from "bencher_valid";
import { Show, createMemo, createResource } from "solid-js";
import type { JsonAlertStats } from "../../../types/bencher";
import { authUser } from "../../../util/auth";
import { httpGet } from "../../../util/http";
import { validJwt } from "../../../util/valid";

interface Props {
	apiUrl: string;
	params: Params;
}

enum Section {
	PERF = "perf",
	REPORTS = "reports",
	BRANCHES = "branches",
	TESTBEDS = "testbeds",
	BENCHMARKS = "benchmarks",
	MEASURES = "measures",
	THRESHOLDS = "thresholds",
	ALERTS = "alerts",
	SETTINGS = "settings",
}

const ProjectMenu = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const params = createMemo(() => props.params);
	const user = authUser();

	const fetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			project_slug: params()?.project,
			token: user?.token,
		};
	});
	const getAlerts = async (fetcher: {
		bencher_valid: InitOutput;
		project_slug: string;
		token: string;
	}): Promise<JsonAlertStats> => {
		const DEFAULT_ALERT_STATS = {
			active: 0,
		};
		if (
			!fetcher.bencher_valid ||
			!fetcher.project_slug ||
			!validJwt(fetcher.token)
		) {
			return DEFAULT_ALERT_STATS;
		}
		const pathname = `/v0/projects/${fetcher.project_slug}/stats/alerts`;
		return await httpGet(props.apiUrl, pathname, authUser()?.token)
			.then((resp) => resp.data)
			.catch((error) => {
				console.error(error);
				return DEFAULT_ALERT_STATS;
			});
	};
	const [alert_stats] = createResource<JsonAlertStats>(fetcher, getAlerts);
	const active_alerts = createMemo(() => alert_stats()?.active);

	const path = (section: Section) =>
		`/console/projects/${params()?.project}/${section}`;

	return (
		<aside class="menu is-sticky">
			<div class="menu-label">
				<a
					class="button is-fullwidth"
					title="View Project Perf"
					href={path(Section.PERF)}
				>
					<span class="icon">
						<i class="fas fa-chart-line" aria-hidden="true" />
					</span>
				</a>
			</div>
			<p class="menu-label">Project</p>
			<ul class="menu-list">
				<li>
					<a href={path(Section.REPORTS)}>Reports</a>
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
					<a href={path(Section.MEASURES)}>Measures</a>
				</li>
				<li>
					<a href={path(Section.THRESHOLDS)}>Thresholds</a>
				</li>
				<li>
					<a href={path(Section.ALERTS)}>
						<nav class="level is-mobile">
							<div class="level-left">
								<div class="level-item">Alerts</div>
								<Show when={active_alerts()}>
									<div class="level-item">
										<button
											class="button is-primary is-small is-rounded"
											type="button"
										>
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

export default ProjectMenu;
