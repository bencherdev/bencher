import "./styles/styles.scss";

import {
  createSignal,
  createEffect,
  lazy,
  Component,
  createMemo,
  Accessor,
  Signal,
} from "solid-js";
import { Routes, Route, Navigate, useLocation } from "solid-app-router";
import { JsonUser } from "bencher_json";

import { Navbar } from "./components/site/navbar/Navbar";
import { GoogleAnalytics } from "./components/site/GoogleAnalytics";

const AuthFormPage = lazy(() => import("./components/auth/AuthFormPage"));
const ConsolePage = lazy(() => import("./components/site/console/ConsolePage"));
const LandingPage = lazy(() => import("./components/site/pages/LandingPage"));

const BENCHER_TITLE = "Bencher";

const App: Component = () => {
  const [title, setTitle] = createSignal<string>(BENCHER_TITLE);
  const [redirect, setRedirect] = createSignal<null | string>();
  const [user, setUser] = createSignal<null | JsonUser>();

  const location = useLocation();
  const current_location = createMemo(() => location);

  createEffect(() => {
    if (document.title !== title()) {
      document.title = title();
    }
  });

  const handleTitle = (new_title) => {
    if (title() !== new_title) {
      setTitle(new_title);
    }
  };

  const getRedirect = () => {
    const new_pathname = redirect();
    if (new_pathname === undefined) {
      return;
    }
    if (new_pathname !== current_location()?.pathname) {
      setRedirect();
      return <Navigate href={new_pathname} />;
    }
  };

  return (
    <>
      <GoogleAnalytics />
      <Navbar />
      {getRedirect()}
      <Routes>
        <Route path="/" element={<LandingPage handleTitle={handleTitle} />} />
        <Route path="/auth">
          <Route
            path="/signup"
            element={
              <AuthFormPage
                kind="signup"
                handleTitle={handleTitle}
                handleRedirect={setRedirect}
                user={user}
                handleUser={setUser}
              />
            }
          />
          <Route
            path="/login"
            element={
              <AuthFormPage
                kind="login"
                handleTitle={handleTitle}
                handleRedirect={setRedirect}
                user={user}
                handleUser={setUser}
              />
            }
          />
        </Route>
        <Route path="/console">
          <Route
            path="/"
            element={
              <ConsolePage
                current_location={current_location}
                handleTitle={handleTitle}
              />
            }
          />
          <Route path="/reports">
            <Route
              path="/"
              element={
                <ConsolePage
                  current_location={current_location}
                  handleTitle={handleTitle}
                  handleRedirect={setRedirect}
                />
              }
            />
            <Route
              path="/:uuid"
              element={
                <ConsolePage
                  current_location={current_location}
                  handleTitle={handleTitle}
                  handleRedirect={setRedirect}
                />
              }
            />
          </Route>
        </Route>
      </Routes>
    </>
  );
};

export default App;
