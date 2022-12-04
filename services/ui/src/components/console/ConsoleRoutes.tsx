import { createMemo, lazy } from "solid-js";
import { Navigate, Route, useParams } from "solid-app-router";
import { Operation, Resource } from "./config/types";

import consoleConfig from "./config/console";

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
      <Route path="/" element={<Navigate href="/console/organizations" />} />
      {/* Console Projects Routes */}
      <Route
        path="/organizations"
        element={consolePage(
          config?.[Resource.ORGANIZATIONS]?.[Operation.LIST]
        )}
      />
      <Route
        path="/organizations/:organization_slug"
        element={<NavigateToProjects />}
      />
      <Route
        path="/organizations/:organization_slug/"
        element={consolePage(
          config?.[Resource.ORGANIZATIONS]?.[Operation.VIEW]
        )}
      />
      <Route
        path="/organizations/:organization_slug/settings"
        element={consolePage(
          config?.[Resource.ORGANIZATIONS]?.[Operation.VIEW]
        )}
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
        path="/projects/:project_slug/thresholds"
        element={consolePage(config?.[Resource.THRESHOLDS]?.[Operation.LIST])}
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
        path="/projects/:project_slug/connections"
        element={consolePage(config?.[Resource.CONNECTIONS]?.[Operation.LIST])}
      />
      <Route
        path="/projects/:project_slug/connections/:connection_uuid"
        element={consolePage(config?.[Resource.CONNECTIONS]?.[Operation.VIEW])}
      />
      <Route
        path="/user/account"
        element={consolePage(config?.[Resource.USER_ACCOUNT]?.[Operation.VIEW])}
      />
      <Route
        path="/user/settings"
        element={consolePage(
          config?.[Resource.USER_SETTINGS]?.[Operation.VIEW]
        )}
      />
    </>
  );
};

export default ConsoleRoutes;

const NavigateToProjects = () => {
  const params = useParams();
  const path_params = createMemo(() => params);

  return (
    <Navigate
      href={`/console/organizations/${
        path_params().organization_slug
      }/projects`}
    />
  );
};
