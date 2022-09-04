import { lazy } from "solid-js";
import { Route, Navigate } from "solid-app-router";

const AuthFormPage = lazy(() => import("./AuthFormPage"));
const AuthLogoutPage = lazy(() => import("./AuthLogoutPage"));

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
            kind="signup"
            config={config[Auth.SIGNUP]}
            handleTitle={props.handleTitle}
            handleRedirect={props.setRedirect}
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
            kind="login"
            config={config[Auth.LOGIN]}
            handleTitle={props.handleTitle}
            handleRedirect={props.setRedirect}
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
            handleTitle={props.handleTitle}
            handleRedirect={props.setRedirect}
            removeUser={props.removeUser}
          />
        }
      />
    </>
  );
};

export default AuthRoutes;
