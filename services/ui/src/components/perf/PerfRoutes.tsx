import { lazy } from "solid-js";
import { Route, Navigate } from "solid-app-router";

const ProjectsPage = lazy(() => import("./ProjectsPage"));
// const AuthLogoutPage = lazy(() => import("./AuthLogoutPage"));
// const AuthConfirmPage = lazy(() => import("./AuthConfirmPage"));

// import AUTH_CONFIG from "./config/auth";
// import { Auth } from "./config/types";

const PerfRoutes = (props) => {
  return (
    <>
      <Route path="/" element={<ProjectsPage user={props.user} />} />
      <Route path="/:project_slug" element={<div>Project Perf Page</div>} />
    </>
  );
};

export default PerfRoutes;
