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

const ConsoleRoutes = (props) => {
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
            current_location={props.current_location}
            handleTitle={props.handleTitle}
            handleRedirect={props.handleRedirect}
          />
        }
      />
      <Route
        path="/projects/:project_slug"
        element={
          <ConsolePage
            operation={Operation.VIEW}
            current_location={props.current_location}
            handleTitle={props.handleTitle}
            handleRedirect={props.handleRedirect}
          />
        }
      />
      <Route
        path="/projects/:project_slug/perf"
        element={
          <ConsolePage
            operation={Operation.PERF}
            current_location={props.current_location}
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
