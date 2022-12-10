import { lazy } from "solid-js";
import { Route } from "solid-app-router";

const ProjectsPage = lazy(() => import("./ProjectsPage"));
const ProjectPage = lazy(() => import("./ProjectPage"));

const PerfRoutes = (props) => {
  return (
    <>
      <Route path="/" element={<ProjectsPage user={props.user} />} />
      <Route
        path="/:project_slug"
        element={<ProjectPage user={props.user} />}
      />
    </>
  );
};

export default PerfRoutes;
