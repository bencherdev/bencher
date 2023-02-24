import { Link, useNavigate } from "solid-app-router";
import {
	is_allowed_organization,
	OrganizationPermission,
} from "../../site/util";

const ConsoleMenu = (props) => {
	const navigate = useNavigate();

	const getOrganizationPath = (section: string) => {
		return `/console/organizations/${props.organization_slug()}/${section}`;
	};

	const getProjectPath = (section: string) => {
		return `/console/projects/${props.project_slug()}/${section}`;
	};

	const getUsersPath = (section: string) => {
		return `/console/users/${props.user?.user?.slug}/${section}`;
	};

	return (
		<aside class="menu">
			{typeof props.organization_slug() === "string" &&
				typeof props.project_slug() !== "string" && (
					<>
						<p class="menu-label">Organization</p>
						<ul class="menu-list">
							<li>
								<Link href={getOrganizationPath("projects")}>Projects</Link>
							</li>
							<li>
								<Link href={getOrganizationPath("members")}>Members</Link>
							</li>
							<li>
								<Link href={getOrganizationPath("settings")}>Settings</Link>
							</li>
							{is_allowed_organization(
								props.path_params(),
								OrganizationPermission.MANAGE,
							) && (
								<li>
									<Link href={getOrganizationPath("billing")}>Billing</Link>
								</li>
							)}
						</ul>
					</>
				)}
			{typeof props.project_slug() === "string" && (
				<>
					<div class="menu-label">
						<button
							class="button is-outlined is-fullwidth"
							onClick={(e) => {
								e.preventDefault();
								navigate(`/console/projects/${props.project_slug()}/perf`);
							}}
						>
							<span class="icon">
								<i class="fas fa-home" aria-hidden="true" />
							</span>
						</button>
					</div>
					<p class="menu-label">Project</p>
					<ul class="menu-list">
						<li>
							<Link href={getProjectPath("reports")}>Reports</Link>
						</li>
						<li>
							<Link href={getProjectPath("metric-kinds")}>Metric Kinds</Link>
						</li>
						<li>
							<Link href={getProjectPath("branches")}>Branches</Link>
						</li>
						<li>
							<Link href={getProjectPath("testbeds")}>Testbeds</Link>
						</li>
						<li>
							<Link href={getProjectPath("benchmarks")}>Benchmarks</Link>
						</li>
						<li>
							<Link href={getProjectPath("thresholds")}>Thresholds</Link>
						</li>
						<li>
							<Link href={getProjectPath("alerts")}>Alerts</Link>
						</li>
						<li>
							<Link href={getProjectPath("settings")}>Settings</Link>
						</li>
					</ul>
				</>
			)}
			<p class="menu-label">User</p>
			<ul class="menu-list">
				<li>
					<Link href={getUsersPath("tokens")}>API Tokens</Link>
				</li>
				<li>
					<Link href={getUsersPath("settings")}>Settings</Link>
				</li>
				<li>
					<Link href={getUsersPath("help")}>Help</Link>
				</li>
			</ul>
		</aside>
	);
};

export default ConsoleMenu;
