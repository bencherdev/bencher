import "./styles/styles.scss";

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

import { Navbar } from "./components/site/navbar/Navbar";
import { GoogleAnalytics } from "./components/site/GoogleAnalytics";
import SiteFooter from "./components/site/pages/SiteFooter";

const AuthFormPage = lazy(() => import("./components/auth/AuthFormPage"));
const AuthLogoutPage = lazy(() => import("./components/auth/AuthLogoutPage"));
const LandingPage = lazy(() => import("./components/site/pages/LandingPage"));
const AccountPage = lazy(() => import("./components/site/account/AccountPage"));
const ConsoleRoutes = lazy(() => import("./components/console/ConsoleRoutes"));

const BENCHER_TITLE = "Bencher";

const initUser = () => {
  return {
    uuid: null,
    name: null,
    slug: null,
    email: null,
    free: null,
  };
};

const initNotification = () => {
  return {
    status: null,
    text: null,
  };
};

const App: Component = () => {
  const [title, setTitle] = createSignal<string>(BENCHER_TITLE);
  const [redirect, setRedirect] = createSignal<null | string>();
  const [user, setUser] = createSignal(initUser());
  const [notification, setNotification] = createSignal(initNotification());

  const location = useLocation();
  const pathname = createMemo(() => location.pathname);

  createEffect(() => {
    if (document.title !== title()) {
      document.title = title();
    }
  });

  const handleUser = (user) => {
    window.localStorage.setItem("user", JSON.stringify(user));
    setUser(user);
  };

  const removeUser = (user) => {
    window.localStorage.clear();
    setUser(initUser());
  };

  const removeNotification = () => {
    setNotification(initNotification());
  };

  const handleNotification = (notification: {
    status: string;
    text: string;
  }) => {
    setNotification(notification);
    setTimeout(() => {
      removeNotification();
    }, 4000);
  };

  setInterval(() => {
    if (user()?.uuid === null) {
      const user = JSON.parse(window.localStorage.getItem("user"));
      if (typeof user?.uuid === "string") {
        setUser(user);
      }
    }
  }, 1000);

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
    if (new_pathname !== pathname()) {
      setRedirect();
      return <Navigate href={new_pathname} />;
    }
  };

  const getNotification = () => {
    let color: string;
    switch (notification().status) {
      case "ok":
        color = "is-success";
        break;
      case "alert":
        color = "is-primary";
        break;
      case "error":
        color = "is-danger";
        break;
      default:
        color = "";
    }
    return (
      <div class={`notification ${color}`}>
        {notification().text}
        <button
          class="delete"
          onClick={(e) => {
            e.preventDefault();
            removeNotification();
          }}
        />
      </div>
    );
  };

  return (
    <>
      <GoogleAnalytics />
      <Navbar user={user} />
      {getRedirect()}

      {notification().text !== null && (
        <section class="section">
          <div class="container">{getNotification()}</div>
        </section>
      )}

      <Routes>
        <Route
          path="/"
          element={
            <LandingPage
              user={user}
              handleTitle={handleTitle}
              handleRedirect={setRedirect}
            />
          }
        />
        {/* Auth Routes */}
        <Route path="/auth">
          <Route
            path="/signup"
            element={
              <AuthFormPage
                kind="signup"
                handleTitle={handleTitle}
                handleRedirect={setRedirect}
                user={user}
                handleUser={handleUser}
                handleNotification={handleNotification}
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
                handleUser={handleUser}
                handleNotification={handleNotification}
              />
            }
          />
          <Route
            path="/logout"
            element={
              <AuthLogoutPage
                handleTitle={handleTitle}
                handleRedirect={setRedirect}
                removeUser={removeUser}
              />
            }
          />
        </Route>

        <Route path="/console">
          <ConsoleRoutes
            pathname={pathname}
            handleTitle={handleTitle}
            handleRedirect={setRedirect}
          />
        </Route>
      </Routes>

      <For each={[...Array(9).keys()]}>{(_k, _i) => <br />}</For>
      <SiteFooter />
    </>
  );
};

export default App;
