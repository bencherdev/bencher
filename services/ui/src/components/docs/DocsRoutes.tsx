import { Route, Navigate } from "solid-app-router";
import docsConfig from "./config/docs";
import Page from "./config/page";
import DocsPage from "./DocsPage";

const DocsRoutes = (props) => {
  const config = docsConfig;

  return (
    <>
      {/* Docs Routes */}
      <Route path="/" element={<Navigate href="/docs/how-to" />} />
      <Route
        path="/how-to"
        element={<Navigate href="/docs/how-to/quick-start" />}
      />
      <Route
        path="/how-to/quick-start"
        element={<DocsPage config={config[Page.QUICK_START]} />}
      />
      <Route
        path="/how-to/github-actions"
        element={<DocsPage config={config[Page.GITHUB_ACTIONS]} />}
      />
      <Route
        path="/how-to/gitlab-ci"
        element={<DocsPage config={config[Page.GITLAB_CI]} />}
      />
      <Route
        path="/how-to/branch-management"
        element={<DocsPage config={config[Page.BRANCH_MANAGEMENT]} />}
      />
      <Route
        path="/reference"
        element={<Navigate href="/docs/reference/api" />}
      />
      <Route
        path="/reference/api"
        element={<Navigate href="/docs/reference/api/v0" />}
      />
      <Route
        path="/reference/api/v0"
        element={<DocsPage config={config[Page.API_V0]} />}
      />
      <Route
        path="/reference/prior-art"
        element={<DocsPage config={config[Page.PRIOR_ART]} />}
      />
      <Route
        path="/reference/changelog"
        element={<DocsPage config={config[Page.CHANGELOG]} />}
      />
    </>
  );
};

export default DocsRoutes;
