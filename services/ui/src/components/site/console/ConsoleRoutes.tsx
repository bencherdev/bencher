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
import { Operation, Resource, Button } from "./console";
import AccountPage from "../account/AccountPage";

const ConsolePage = lazy(() => import("./ConsolePage"));

const config = {
  [Resource.PROJECTS]: {
    [Operation.LIST]: {
      operation: Operation.LIST,
      title: "Projects",
      header: "name",
      items: [
        {
          kind: "text",
          key: "slug",
        },
        {},
        {
          kind: "bool",
          key: "owner_default",
          text: "Default",
        },
        {},
      ],
      buttons: [Button.ADD, Button.REFRESH],
    },
  },
};

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
            config={config[Resource.PROJECTS][Operation.LIST]}
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
            config={config[Resource.PROJECTS][Operation.VIEW]}
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
            config={config[Resource.PROJECTS][Operation.PERF]}
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
