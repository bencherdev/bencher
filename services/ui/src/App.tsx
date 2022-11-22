import "./styles/styles.scss";

import {
  createSignal,
  createEffect,
  lazy,
  Component,
  createMemo,
  For,
  createResource,
} from "solid-js";
import { Routes, Route, useLocation } from "solid-app-router";
import init from "bencher_valid";

import { Navbar } from "./components/site/navbar/Navbar";
import SiteFooter from "./components/site/pages/SiteFooter";
import { projectSlug } from "./components/console/ConsolePage";
import { BENCHER_USER_KEY, NotifyKind } from "./components/site/util";
import validator from "validator";

const AuthRoutes = lazy(() => import("./components/auth/AuthRoutes"));
const LandingPage = lazy(() => import("./components/site/pages/LandingPage"));
const PricingPage = lazy(() => import("./components/site/pages/PricingPage"));
const ConsoleRoutes = lazy(() => import("./components/console/ConsoleRoutes"));
const DocsRoutes = lazy(() => import("./components/docs/DocsRoutes"));
const LegalRoutes = lazy(() => import("./components/legal/LegalRoutes"));
const Repo = lazy(() => import("./components/site/Repo"));

const initUser = () => {
  return {
    user: {
      uuid: null,
      name: null,
      slug: null,
      email: null,
      admin: null,
      locked: null,
    },
    token: null,
  };
};

const App: Component = () => {
  const location = useLocation();
  const pathname = createMemo(() => location.pathname);
  const [_wasm] = createResource(pathname, () => init());

  const [organization_slug, setOrganizationSlug] = createSignal<null | String>(
    null
  );
  // The project slug can't be a resource because it isn't 100% tied to the URL
  const [project_slug, setProjectSlug] = createSignal<String>(
    projectSlug(pathname)
  );

  const [user, setUser] = createSignal(initUser());

  const handleUser = (user) => {
    window.localStorage.setItem(BENCHER_USER_KEY, JSON.stringify(user));
    setUser(user);
  };

  const removeUser = () => {
    window.localStorage.clear();
    setUser(initUser());
  };

  createEffect(() => {
    if (user()?.token === null) {
      const user = JSON.parse(window.localStorage.getItem(BENCHER_USER_KEY));
      // TODO properly validate entire user
      if (user?.token && validator.isJWT(user.token)) {
        setUser(user);
      }
    }
  });

  return (
    <>
      <Navbar
        user={user}
        organization_slug={organization_slug}
        project_slug={project_slug}
        handleProjectSlug={setProjectSlug}
      />

      <Routes>
        <Route path="/" element={<LandingPage user={user} />} />

        {/* Auth Routes */}
        <Route path="/auth">
          <AuthRoutes
            user={user}
            handleUser={handleUser}
            removeUser={removeUser}
          />
        </Route>

        {/* Console Routes */}
        <Route path="/console">
          <ConsoleRoutes
            user={user}
            pathname={pathname}
            organization_slug={organization_slug}
            project_slug={project_slug}
            handleOrganizationSlug={setOrganizationSlug}
            handleProjectSlug={setProjectSlug}
          />
        </Route>

        {/* Docs Routes */}
        <Route path="/docs">
          <DocsRoutes />
        </Route>

        {/* Auth Routes */}
        <Route path="/legal">
          <LegalRoutes />
        </Route>

        {/* GitHub repo shortcut */}
        <Route path="/repo" element={<Repo />} />

        {/* Pricing Routes */}
        <Route path="/pricing" element={<PricingPage user={user} />} />
      </Routes>

      <For each={[...Array(16).keys()]}>{(_k, _i) => <br />}</For>
      <SiteFooter />
    </>
  );
};

export default App;
