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
const AuthLogoutPage = lazy(() => import("./components/auth/AuthLogoutPage"));
const ConsolePage = lazy(() => import("./components/site/console/ConsolePage"));
const LandingPage = lazy(() => import("./components/site/pages/LandingPage"));

const BENCHER_TITLE = "Bencher";

const initUser = () => {
  return {
    id: null,
    username: null,
    email: null,
    isAuth: false,
    isConf: false,
    permissions: {},
    role: {
      value: null,
      permissions: 0,
    },
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
  const current_location = createMemo(() => location);

  createEffect(() => {
    if (document.title !== title()) {
      document.title = title();
    }
  });

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

  // setInterval(() => setCount(count() + 1), 1000);

  // setInterval(() => {
  //   if (!user.isAuth && window.localStorage.getItem("authToken")) {
  //     getStatus();
  //   }
  //   setCount(count + 1);
  // }, 5000);

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
        ></button>
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
                handleUser={setUser}
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
                handleUser={setUser}
                handleNotification={handleNotification}
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
