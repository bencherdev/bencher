import { Route, Navigate } from "solid-app-router";
import SwaggerPanel from "./api/SwaggerPanel";
import docsConfig from "./config/docs";
import { Docs } from "./config/types";
import DocsMenu from "./DocsMenu";
import DocsPage from "./DocsPage";
import Message from "./example.mdx";

const DocsRoutes = (props) => {
  const config = docsConfig;

  const docsPage = (page) => {
    return <DocsPage page={page} />;
  };

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
    </>
  );
};

export default DocsRoutes;
