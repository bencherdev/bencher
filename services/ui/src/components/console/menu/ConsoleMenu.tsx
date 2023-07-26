import { Link, useNavigate } from "solid-app-router";
import {
	BENCHER_API_URL,
	get_options,
	is_allowed_organization,
	OrganizationPermission,
	validate_jwt,
} from "../../site/util";
import {
	Match,
	Show,
	Switch,
	createEffect,
	createMemo,
	createResource,
} from "solid-js";
import { JsonAlertStats, JsonAlertStatus } from "../../../types/bencher";
import axios from "axios";
import { create } from "domain";

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

const ConsoleMenu = (props) => {
	const navigate = useNavigate();

	const getOne = async (project_slug) => {
		const EMPTY_OBJECT = {};
		const token = props.user?.token;
		if (!validate_jwt(token)) {
			return EMPTY_OBJECT;
		}
		const url = `${BENCHER_API_URL()}/v0/projects/${project_slug}/stats/alerts`;
		return await axios(get_options(url, token))
			.then((resp) => resp.data)
			.catch((error) => {
				console.error(error);
				return EMPTY_OBJECT;
			});
	};
	const [alert_stats] = createResource(props.project_slug, getOne);
	const active = createMemo(() => alert_stats()?.active);

	const organizations_path = (section: OrganizationSection) => {
		return `/console/organizations/${props.organization_slug()}/${section}`;
	};

	const projects_path = (section: ProjectSection) => {
		return `/console/projects/${props.project_slug()}/${section}`;
	};

	const users_path = (section: UserSection) => {
		return `/console/users/${props.user?.user?.slug}/${section}`;
	};

	return (
		<aside class="menu">
			<Switch fallback={<></>}>
				<Match
					when={
						typeof props.organization_slug() === "string" &&
						typeof props.project_slug() !== "string"
					}
				>
					<>
						<p class="menu-label">Organization</p>
						<ul class="menu-list">
							<li>
								<Link href={organizations_path(OrganizationSection.PROJECTS)}>
									Projects
								</Link>
							</li>
							<li>
								<Link href={organizations_path(OrganizationSection.MEMBERS)}>
									Members
								</Link>
							</li>
							<li>
								<Link href={organizations_path(OrganizationSection.SETTINGS)}>
									Settings
								</Link>
							</li>
							<Show
								when={is_allowed_organization(
									props.path_params,
									OrganizationPermission.MANAGE,
								)}
								fallback={<></>}
							>
								<li>
									<Link href={organizations_path(OrganizationSection.BILLING)}>
										Billing
									</Link>
								</li>
							</Show>
						</ul>
					</>
				</Match>
				<Match when={typeof props.project_slug() === "string"}>
					<>
						<div class="menu-label">
							<button
								class="button is-outlined is-fullwidth"
								title="View Project Perf"
								onClick={(e) => {
									e.preventDefault();
									navigate(projects_path(ProjectSection.PERF));
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
								<Link href={projects_path(ProjectSection.REPORTS)}>
									Reports
								</Link>
							</li>
							<li>
								<Link href={projects_path(ProjectSection.METRIC_KINDS)}>
									Metric Kinds
								</Link>
							</li>
							<li>
								<Link href={projects_path(ProjectSection.BRANCHES)}>
									Branches
								</Link>
							</li>
							<li>
								<Link href={projects_path(ProjectSection.TESTBEDS)}>
									Testbeds
								</Link>
							</li>
							<li>
								<Link href={projects_path(ProjectSection.BENCHMARKS)}>
									Benchmarks
								</Link>
							</li>
							<li>
								<Link href={projects_path(ProjectSection.THRESHOLDS)}>
									Thresholds
								</Link>
							</li>
							<li>
								<Link href={projects_path(ProjectSection.ALERTS)}>
									<nav class="level">
										<div class="level-left">
											<div class="level-item">Alerts</div>
											<Show when={active()} fallback={<></>}>
												<div class="level-item">
													<button class="button is-primary is-small is-rounded">
														{active()}
													</button>
												</div>
											</Show>
										</div>
									</nav>
								</Link>
							</li>
							<li>
								<Link href={projects_path(ProjectSection.SETTINGS)}>
									Settings
								</Link>
							</li>
						</ul>
					</>
				</Match>
			</Switch>
			<p class="menu-label">User</p>
			<ul class="menu-list">
				<li>
					<Link href={users_path(UserSection.TOKENS)}>API Tokens</Link>
				</li>
				<li>
					<Link href={users_path(UserSection.SETTINGS)}>Settings</Link>
				</li>
				<li>
					<Link href={users_path(UserSection.HELP)}>Help</Link>
				</li>
			</ul>
		</aside>
	);
};

export default ConsoleMenu;
