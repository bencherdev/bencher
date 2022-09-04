import { lazy } from "solid-js";
import { Route, Navigate } from "solid-app-router";

const AuthFormPage = lazy(() => import("./AuthFormPage"));
const AuthLogoutPage = lazy(() => import("./AuthLogoutPage"));

const AuthRoutes = (props) => {
  return (
    <>
      <Route path="/" element={<Navigate href="/auth/signup" />} />
      <Route
        path="/signup"
        element={
          <AuthFormPage
            kind="signup"
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
