import { lazy, createResource } from "solid-js";
import { Routes, Route, Navigate, useLocation } from "solid-app-router";
import { Operation, Resource, Button, Field } from "./console";
import AccountPage from "../site/account/AccountPage";

import consoleConfig from "./config/console";

const ConsolePage = lazy(() => import("./ConsolePage"));

const ConsoleRoutes = (props) => {
  const [config] = createResource(props.pathname, consoleConfig);

  return (
    <>
      {/* Console Routes */}
      <Route path="/" element={<Navigate href={"/console/projects"} />} />
      {/* Console Projects Routes */}
      <Route
        path="/projects"
        element={
          <ConsolePage
            config={config()?.[Resource.PROJECTS]?.[Operation.LIST]}
            pathname={props.pathname}
            handleTitle={props.handleTitle}
            handleRedirect={props.handleRedirect}
          />
        }
      />
      <Route
        path="/projects/add"
        element={
          <ConsolePage
            config={config()?.[Resource.PROJECTS]?.[Operation.ADD]}
            pathname={props.pathname}
            handleTitle={props.handleTitle}
            handleRedirect={props.handleRedirect}
          />
        }
      />
      <Route
        path="/projects/:project_slug"
        element={
          <ConsolePage
            config={config()?.[Resource.PROJECTS]?.[Operation.VIEW]}
            pathname={props.pathname}
            handleTitle={props.handleTitle}
            handleRedirect={props.handleRedirect}
          />
        }
      />
      <Route
        path="/projects/:project_slug/perf"
        element={
          <ConsolePage
            config={config()?.[Resource.PROJECTS]?.[Operation.PERF]}
            pathname={props.pathname}
            handleTitle={props.handleTitle}
            handleRedirect={props.handleRedirect}
          />
        }
      />
      <Route path="/account" element={<AccountPage />} />
    </>
  );
};

export default ConsoleRoutes;
