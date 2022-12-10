import { lazy } from "solid-js";
import { Route, Navigate } from "solid-app-router";

// const AuthFormPage = lazy(() => import("./AuthFormPage"));
// const AuthLogoutPage = lazy(() => import("./AuthLogoutPage"));
// const AuthConfirmPage = lazy(() => import("./AuthConfirmPage"));

// import AUTH_CONFIG from "./config/auth";
// import { Auth } from "./config/types";

const PerfRoutes = (props) => {
  return (
    <>
      <Route path="/" element={<div>List top projects</div>} />
      <Route path="/:project_slug" element={<div>Project Perf Page</div>} />
    </>
  );
};

export default PerfRoutes;
