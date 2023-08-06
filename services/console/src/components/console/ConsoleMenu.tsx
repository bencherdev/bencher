import { Match, Show, Switch, createMemo, createResource } from "solid-js";
import { organizationSlug, projectSlug, useNavigate } from "../../util/url";
import { authUser } from "../../util/auth";
import { BENCHER_API_URL } from "../../util/ext";
import { httpGet } from "../../util/http";
import type { JsonAlertStats } from "../../types/bencher";

enum OrganizationSection {
	PROJECTS = "projects",
	MEMBERS = "members",
	SETTINGS = "settings",
	BILLING = "billing",
}

enum ProjectSection {
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

enum UserSection {
	TOKENS = "tokens",
	SETTINGS = "settings",
	HELP = "help",
}

const ConsoleMenu = () => {
	const navigate = useNavigate();

	const getAlerts = async (
		project_slug: null | string,
	): Promise<JsonAlertStats> => {
		const DEFAULT_ALERT_STATS = {
			active: 0,
		};
		if (!project_slug) {
			return DEFAULT_ALERT_STATS;
		}
		const url = `${BENCHER_API_URL()}/v0/projects/${project_slug}/stats/alerts`;
		return await httpGet(url, authUser()?.token)
			.then((resp) => resp.data)
			.catch((error) => {
				console.error(error);
				return DEFAULT_ALERT_STATS;
			});
	};
	const [alert_stats] = createResource(projectSlug, getAlerts);
	const active_alerts = createMemo(() => alert_stats()?.active);

	const organizationsPath = (section: OrganizationSection) =>
		`/console/organizations/${organizationSlug()}/${section}`;
	const projectsPath = (section: ProjectSection) =>
		`/console/projects/${projectSlug()}/${section}`;
	const usersPath = (section: UserSection) =>
		`/console/users/${authUser()?.user?.slug}/${section}`;

	return (
		<aside class="menu">
			<Switch fallback={<></>}>
				<Match
					when={
						typeof organizationSlug() === "string" &&
						typeof projectSlug() !== "string"
					}
				>
					<>
						<p class="menu-label">Organization</p>
						<ul class="menu-list">
							<li>
								<a href={organizationsPath(OrganizationSection.PROJECTS)}>
									Projects
								</a>
							</li>
							<li>
								<a href={organizationsPath(OrganizationSection.MEMBERS)}>
									Members
								</a>
							</li>
							<li>
								<a href={organizationsPath(OrganizationSection.SETTINGS)}>
									Settings
								</a>
							</li>
							{/* <Show
								when={is_allowed_organization(
									props.path_params,
									OrganizationPermission.MANAGE,
								)}
								fallback={<></>}
							>
								<li>
									<a href={organizationsPath(OrganizationSection.BILLING)}>
										Billing
									</a>
								</li>
							</Show> */}
						</ul>
					</>
				</Match>
				<Match when={typeof projectSlug() === "string"}>
					<>
						<div class="menu-label">
							<button
								class="button is-outlined is-fullwidth"
								title="View Project Perf"
								onClick={(e) => {
									e.preventDefault();
									navigate(projectsPath(ProjectSection.PERF));
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
								<a href={projectsPath(ProjectSection.REPORTS)}>Reports</a>
							</li>
							<li>
								<a href={projectsPath(ProjectSection.METRIC_KINDS)}>
									Metric Kinds
								</a>
							</li>
							<li>
								<a href={projectsPath(ProjectSection.BRANCHES)}>Branches</a>
							</li>
							<li>
								<a href={projectsPath(ProjectSection.TESTBEDS)}>Testbeds</a>
							</li>
							<li>
								<a href={projectsPath(ProjectSection.BENCHMARKS)}>Benchmarks</a>
							</li>
							<li>
								<a href={projectsPath(ProjectSection.THRESHOLDS)}>Thresholds</a>
							</li>
							<li>
								<a href={projectsPath(ProjectSection.ALERTS)}>
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
								<a href={projectsPath(ProjectSection.SETTINGS)}>Settings</a>
							</li>
						</ul>
					</>
				</Match>
			</Switch>
			<p class="menu-label">User</p>
			<ul class="menu-list">
				<li>
					<a href={usersPath(UserSection.TOKENS)}>API Tokens</a>
				</li>
				<li>
					<a href={usersPath(UserSection.SETTINGS)}>Settings</a>
				</li>
				<li>
					<a href={usersPath(UserSection.HELP)}>Help</a>
				</li>
			</ul>
		</aside>
	);
};

export default ConsoleMenu;
