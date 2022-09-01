import { lazy } from "solid-js";
import { Route, Navigate } from "solid-app-router";
import SwaggerUI from "swagger-ui";

import swagger from "./api/swagger.json";

const Example = lazy(() => import("./Example.mdx"));

const DocsRoutes = (props) => {
  const docsPage = () => {
    return <Example />;
  };

  return (
    <>
      {/* Docs Routes */}
      <Route path="/" element={<Navigate href="/docs/quick-start" />} />
      {/* Console Projects Routes */}
      <Route path="/quick-start" element={docsPage()} />
      <Route path="/api" element={<Navigate href="/docs/api/v0" />} />
      <Route
        path="/api/v0"
        element={SwaggerUI({
          dom_id: "#root",
          spec: swagger,
          layout: "BaseLayout",
        })}
      />
    </>
  );
};

export default DocsRoutes;
