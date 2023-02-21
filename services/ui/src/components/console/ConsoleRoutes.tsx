import { createEffect, createMemo, lazy } from "solid-js";
import {
	Navigate,
	Route,
	useNavigate,
	useParams,
	useSearchParams,
} from "solid-app-router";
import { Operation, Resource } from "./config/types";

import consoleConfig from "./config/console";
import Forward, { forward_path } from "../site/Forward";
import { PLAN_PARAM } from "../auth/AuthForm";
import {
	NOTIFY_KIND_PARAM,
	NOTIFY_TEXT_PARAM,
	validate_plan_level,
} from "../site/util";
import { Host } from "./config/resources/billing";

const ConsolePage = lazy(() => import("./ConsolePage"));

const ConsoleRoutes = (props) => {
	const config = consoleConfig;
	const consolePage = (config) => {
		return (
			<ConsolePage
				user={props.user}
				config={config}
				organization_slug={props.organization_slug}
				project_slug={props.project_slug}
				handleOrganizationSlug={props.handleOrganizationSlug}
				handleProjectSlug={props.handleProjectSlug}
			/>
		);
	};

	return (
		<>
			{/* Console Routes */}
			<Route path="/" element={<NavigateToOrganizations />} />
			{/* Console Projects Routes */}
			<Route
				path="/organizations"
				element={consolePage(
					config?.[Resource.ORGANIZATIONS]?.[Operation.LIST],
				)}
			/>
			<Route
				path="/organizations/:organization_slug"
				element={<NavigateToOrganization />}
			/>
			<Route
				path="/organizations/:organization_slug/"
				element={consolePage(
					config?.[Resource.ORGANIZATIONS]?.[Operation.VIEW],
				)}
			/>
			<Route
				path="/organizations/:organization_slug/settings"
				element={consolePage(
					config?.[Resource.ORGANIZATIONS]?.[Operation.VIEW],
				)}
			/>
			<Route
				path="/organizations/:organization_slug/billing"
				element={consolePage(config?.[Resource.BILLING]?.[Host.BENCHER_CLOUD])}
			/>
			<Route
				path="/organizations/:organization_slug/projects"
				element={consolePage(config?.[Resource.PROJECTS]?.[Operation.LIST])}
			/>
			<Route
				path="/organizations/:organization_slug/projects/add"
				element={consolePage(config?.[Resource.PROJECTS]?.[Operation.ADD])}
			/>
			<Route
				path="/organizations/:organization_slug/projects/:project_slug"
				element={consolePage(config?.[Resource.PROJECTS]?.[Operation.VIEW])}
			/>
			<Route
				path="/organizations/:organization_slug/members"
				element={consolePage(config?.[Resource.MEMBERS]?.[Operation.LIST])}
			/>
			<Route
				path="/organizations/:organization_slug/members/invite"
				element={consolePage(config?.[Resource.MEMBERS]?.[Operation.ADD])}
			/>
			<Route
				path="/organizations/:organization_slug/members/:member_slug"
				element={consolePage(config?.[Resource.MEMBERS]?.[Operation.VIEW])}
			/>
			<Route path="/projects/:project_slug" element={<NavigateToProject />} />
			<Route
				path="/projects/:project_slug/settings"
				element={consolePage(config?.[Resource.PROJECTS]?.[Operation.VIEW])}
			/>
			<Route
				path="/projects/:project_slug/perf"
				element={consolePage(config?.[Resource.PROJECTS]?.[Operation.PERF])}
			/>
			<Route
				path="/projects/:project_slug/reports"
				element={consolePage(config?.[Resource.REPORTS]?.[Operation.LIST])}
			/>
			<Route
				path="/projects/:project_slug/reports/:report_uuid"
				element={consolePage(config?.[Resource.REPORTS]?.[Operation.VIEW])}
			/>
			<Route
				path="/projects/:project_slug/metric-kinds"
				element={consolePage(config?.[Resource.METRIC_KINDS]?.[Operation.LIST])}
			/>
			<Route
				path="/projects/:project_slug/metric-kinds/add"
				element={consolePage(config?.[Resource.METRIC_KINDS]?.[Operation.ADD])}
			/>
			<Route
				path="/projects/:project_slug/metric-kinds/:metric_kind_slug"
				element={consolePage(config?.[Resource.METRIC_KINDS]?.[Operation.VIEW])}
			/>
			<Route
				path="/projects/:project_slug/branches"
				element={consolePage(config?.[Resource.BRANCHES]?.[Operation.LIST])}
			/>
			<Route
				path="/projects/:project_slug/branches/add"
				element={consolePage(config?.[Resource.BRANCHES]?.[Operation.ADD])}
			/>
			<Route
				path="/projects/:project_slug/branches/:branch_slug"
				element={consolePage(config?.[Resource.BRANCHES]?.[Operation.VIEW])}
			/>
			<Route
				path="/projects/:project_slug/testbeds"
				element={consolePage(config?.[Resource.TESTBEDS]?.[Operation.LIST])}
			/>
			<Route
				path="/projects/:project_slug/testbeds/add"
				element={consolePage(config?.[Resource.TESTBEDS]?.[Operation.ADD])}
			/>
			<Route
				path="/projects/:project_slug/testbeds/:testbed_slug"
				element={consolePage(config?.[Resource.TESTBEDS]?.[Operation.VIEW])}
			/>
			<Route
				path="/projects/:project_slug/benchmarks"
				element={consolePage(config?.[Resource.BENCHMARKS]?.[Operation.LIST])}
			/>
			<Route
				path="/projects/:project_slug/benchmarks/:benchmark_uuid"
				element={consolePage(config?.[Resource.BENCHMARKS]?.[Operation.VIEW])}
			/>
			<Route
				path="/projects/:project_slug/thresholds"
				element={consolePage(config?.[Resource.THRESHOLDS]?.[Operation.LIST])}
			/>
			<Route
				path="/projects/:project_slug/thresholds"
				element={consolePage(config?.[Resource.METRIC_KINDS]?.[Operation.LIST])}
			/>
			<Route
				path="/projects/:project_slug/thresholds/add"
				element={consolePage(config?.[Resource.THRESHOLDS]?.[Operation.ADD])}
			/>
			<Route
				path="/projects/:project_slug/thresholds/:threshold_uuid"
				element={consolePage(config?.[Resource.THRESHOLDS]?.[Operation.VIEW])}
			/>
			<Route
				path="/projects/:project_slug/alerts"
				element={consolePage(config?.[Resource.ALERTS]?.[Operation.LIST])}
			/>
			<Route
				path="/projects/:project_slug/alerts/:alert_uuid"
				element={consolePage(config?.[Resource.ALERTS]?.[Operation.VIEW])}
			/>
			<Route
				path="/users/:user_slug/settings"
				element={consolePage(config?.[Resource.USERS]?.[Operation.VIEW])}
			/>
			<Route
				path="/users/:user_slug/tokens"
				element={consolePage(config?.[Resource.TOKENS]?.[Operation.LIST])}
			/>
			<Route
				path="/users/:user_slug/tokens/add"
				element={consolePage(config?.[Resource.TOKENS]?.[Operation.ADD])}
			/>
			<Route
				path="/users/:user_slug/tokens/:token_uuid"
				element={consolePage(config?.[Resource.TOKENS]?.[Operation.VIEW])}
			/>
			<Route path="/billing" element={<div>HERE</div>} />
		</>
	);
};

export default ConsoleRoutes;

const NavigateToOrganizations = () => {
	const navigate = useNavigate();
	createEffect(() => {
		navigate(
			forward_path(
				"/console/organizations",
				[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM, PLAN_PARAM],
				[],
			),
		);
	});
	return <></>;
};

const NavigateToOrganization = () => {
	const [searchParams, setSearchParams] = useSearchParams();
	const params = useParams();
	const path_params = createMemo(() => params);
	const navigate = useNavigate();

	if (!validate_plan_level(searchParams[PLAN_PARAM])) {
		setSearchParams({ [PLAN_PARAM]: null });
	}
	const plan = createMemo(() => searchParams[PLAN_PARAM]);

	const org_section = () => {
		console.log(`PLAN ${plan()}`);
		if (plan()) {
			return "billing";
		} else {
			return "projects";
		}
	};

	createEffect(() => {
		navigate(
			forward_path(
				`/console/organizations/${
					path_params().organization_slug
				}/${org_section()}`,
				[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM, PLAN_PARAM],
				[],
			),
		);
	});
	return <></>;
};

const NavigateToProject = () => {
	const params = useParams();
	const path_params = createMemo(() => params);

	return (
		<Navigate href={`/console/projects/${path_params().project_slug}/perf`} />
	);
};
