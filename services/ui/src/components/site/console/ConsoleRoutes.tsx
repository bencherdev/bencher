import {
  createSignal,
  createEffect,
  lazy,
  Component,
  createMemo,
  Accessor,
  Signal,
  For,
  createResource,
} from "solid-js";
import { Routes, Route, Navigate, useLocation } from "solid-app-router";
import { JsonUser } from "bencher_json";
import { Operation, Resource, Button, Field } from "./console";
import AccountPage from "../account/AccountPage";

const ConsolePage = lazy(() => import("./ConsolePage"));

const getConfig = (pathname) => {
  console.log(pathname);
  return {
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
        buttons: [
          { kind: Button.ADD, path: "/console/projects/add" },
          { kind: Button.REFRESH },
        ],
      },
      [Operation.ADD]: {
        operation: Operation.ADD,
        title: "Add Project",
        fields: [
          {
            kind: Field.INPUT,
            key: "name",
            label: true,
            value: "",
            valid: null,
            validate: true,
            clear: false,
            config: {
              label: "Project Name",
              type: "text",
              placeholder: "Project Name",
              icon: "fas fa-project-diagram",
              disabled: false,
            },
          },
        ],
        buttons: {
          [Button.BACK]: { path: "/console/projects" },
        },
      },
    },
  };
};

const ConsoleRoutes = (props) => {
  const [config] = createResource(props.pathname, getConfig);

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
