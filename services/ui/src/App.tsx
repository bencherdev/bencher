import "./styles/styles.scss";

import { createSignal, createEffect, lazy, Component } from "solid-js";
import { Routes, Route } from "solid-app-router";

import { Navbar } from "./components/site/navbar/Navbar";
import { GoogleAnalytics } from "./components/site/GoogleAnalytics";

const AuthFormPage = lazy(() => import("./components/auth/AuthFormPage"));
const ConsolePage = lazy(() => import("./components/site/console/ConsolePage"));
const LandingPage = lazy(() => import("./components/site/pages/LandingPage"));

const BENCHER_TITLE = "Bencher";

const App: Component = () => {
  const [title, setTitle] = createSignal(BENCHER_TITLE);

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

  return (
    <>
      <GoogleAnalytics />
      <Navbar />
      <Routes>
        <Route path="/" element={<LandingPage handleTitle={handleTitle} />} />
        <Route path="/auth">
          <Route
            path="/signup"
            element={<AuthFormPage kind="signup" handleTitle={handleTitle} />}
          />
          <Route
            path="/login"
            element={<AuthFormPage kind="login" handleTitle={handleTitle} />}
          />
        </Route>
        <Route path="/console">
          <Route path="/" element={<ConsolePage handleTitle={handleTitle} />} />
          <Route
            path="/reports"
            element={<ConsolePage handleTitle={handleTitle} />}
          />
        </Route>
      </Routes>
    </>
  );
};

export default App;
