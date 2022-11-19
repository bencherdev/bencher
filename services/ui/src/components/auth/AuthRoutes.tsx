import { createEffect, lazy } from "solid-js";
import { Route, Navigate } from "solid-app-router";

const AuthFormPage = lazy(() => import("./AuthFormPage"));
const AuthLogoutPage = lazy(() => import("./AuthLogoutPage"));
const AuthConfirmPage = lazy(() => import("./AuthConfirmPage"));

import authConfig from "./config/auth";
import { Auth } from "./config/types";

const AuthRoutes = (props) => {
  const config = authConfig;

  return (
    <>
      <Route path="/" element={<Navigate href="/auth/signup" />} />
      <Route
        path="/signup"
        element={
          <AuthFormPage
            config={config[Auth.SIGNUP]}
            user={props.user}
            handleUser={props.handleUser}
            handleNotification={props.handleNotification}
          />
        }
      />
      <Route
        path="/login"
        element={
          <AuthFormPage
            config={config[Auth.LOGIN]}
            user={props.user}
            handleUser={props.handleUser}
            handleNotification={props.handleNotification}
          />
        }
      />
      <Route
        path="/confirm"
        element={
          <AuthConfirmPage
            config={config[Auth.CONFIRM]}
            user={props.user}
            handleUser={props.handleUser}
            handleNotification={props.handleNotification}
          />
        }
      />
      <Route
        path="/logout"
        element={
          <AuthLogoutPage
            config={config[Auth.LOGOUT]}
            removeUser={props.removeUser}
            handleNotification={props.handleNotification}
          />
        }
      />
    </>
  );
};

export default AuthRoutes;
