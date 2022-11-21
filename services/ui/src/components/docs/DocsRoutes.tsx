import { Route, Navigate } from "solid-app-router";
import docsConfig from "./config/docs";
import { Docs } from "./config/types";
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
        element={<DocsPage config={config[Docs.QUICK_START]} />}
      />
      {/* <Route path="/how-to/run-a-report" element={<p>TODO</p>} /> */}
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
        element={<DocsPage config={config[Docs.API_V0]} />}
      />
      <Route
        path="/reference/changelog"
        element={<DocsPage config={config[Docs.CHANGELOG]} />}
      />
    </>
  );
};

export default DocsRoutes;
