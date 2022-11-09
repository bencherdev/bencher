import { Route, Navigate } from "solid-app-router";
import DocsPage from "./DocsPage";

const DocsRoutes = (props) => {
  const docsPage = (page) => {
    return <DocsPage page={page} />;
  };

  return (
    <>
      {/* Docs Routes */}
      {/* <Route path="/" element={<Navigate href="/docs/how-to" />} />
      <Route
        path="/how-to"
        element={<Navigate href="/docs/how-to/quick-start" />}
      />
      <Route path="/how-to/quick-start" element={docsPage(true)} />
      <Route path="/how-to/run-a-report" element={docsPage(true)} /> */}
      <Route
        path="/reference"
        element={<Navigate href="/docs/reference/api" />}
      />
      <Route
        path="/reference/api"
        element={<Navigate href="/docs/reference/api/v0" />}
      />
      <Route path="/reference/api/v0" element={docsPage(false)} />
    </>
  );
};

export default DocsRoutes;
