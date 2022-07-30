import {
  createSignal,
  createEffect,
  lazy,
  Component,
  createMemo,
  Accessor,
  Signal,
  For,
} from "solid-js";
import { Routes, Route, Navigate, useLocation } from "solid-app-router";
import { JsonUser } from "bencher_json";
import { Operation } from "./console";
import AccountPage from "../account/AccountPage";

const ConsolePage = lazy(() => import("./ConsolePage"));

const initSlug = (current_location) => {
  const path = current_location().pathname?.split("/");
  if (
    path.length < 5 ||
    path[0] ||
    path[1] !== "console" ||
    path[2] !== "projects" ||
    !path[3]
  ) {
    return null;
  }
  return path[3];
};

const ConsoleRoutes = (props) => {
  const [project_slug, setProjectSlug] = createSignal<String>(
    initSlug(props.current_location)
  );

  return (
    <>
      {/* Console Routes */}
      <Route path="/" element={<Navigate href={"/console/projects"} />} />
      {/* Console Projects Routes */}
      <Route
        path="/projects"
        element={
          <ConsolePage
            operation={Operation.LIST}
            project_slug={project_slug}
            current_location={props.current_location}
            handleTitle={props.handleTitle}
            handleRedirect={props.handleRedirect}
            handleProjectSlug={setProjectSlug}
          />
        }
      />
      <Route
        path="/projects/:project_slug"
        element={
          <ConsolePage
            operation={Operation.VIEW}
            project_slug={project_slug}
            current_location={props.current_location}
            handleTitle={props.handleTitle}
            handleRedirect={props.handleRedirect}
            handleProjectSlug={setProjectSlug}
          />
        }
      />
      <Route
        path="/projects/:project_slug/perf"
        element={
          <ConsolePage
            operation={Operation.PERF}
            project_slug={project_slug}
            current_location={props.current_location}
            handleTitle={props.handleTitle}
            handleRedirect={props.handleRedirect}
            handleProjectSlug={setProjectSlug}
          />
        }
      />
      <Route path="/account" element={<AccountPage />} />
    </>
  );
};

export default ConsoleRoutes;
