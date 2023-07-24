import "./styles/styles.scss";

import { createSignal, lazy, Component, createMemo, For } from "solid-js";
import { Routes, Route, useLocation } from "solid-app-router";
import { Navbar } from "./components/site/navbar/Navbar";
import Footer from "./components/site/pages/Footer";
import { projectSlug } from "./components/console/ConsolePage";
import {
	BENCHER_USER_KEY,
	default_user,
	load_user,
	validate_user,
} from "./components/site/util";
import { createStore } from "solid-js/store";

const AuthRoutes = lazy(() => import("./components/auth/AuthRoutes"));
const LandingPage = lazy(() => import("./components/site/pages/LandingPage"));
const PricingPage = lazy(() => import("./components/site/pages/PricingPage"));
const ConsoleRoutes = lazy(() => import("./components/console/ConsoleRoutes"));
const DocsRoutes = lazy(() => import("./components/docs/DocsRoutes"));
const PerfRoutes = lazy(() => import("./components/perf/PerfRoutes"));
const LegalRoutes = lazy(() => import("./components/legal/LegalRoutes"));
const Repo = lazy(() => import("./components/site/pages/Repo"));
const Demo = lazy(() => import("./components/site/pages/Demo"));

const App: Component = () => {
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	const [organization_slug, setOrganizationSlug] = createSignal<null | String>(
		null,
	);
	// The project slug can't be a resource because it isn't 100% tied to the URL
	const [project_slug, setProjectSlug] = createSignal<String>(
		projectSlug(pathname),
	);

	const [user, setUser] = createStore(load_user());

	const handleUser = (user): boolean => {
		if (validate_user(user)) {
			window.localStorage.setItem(BENCHER_USER_KEY, JSON.stringify(user));
			setUser(user);
			return true;
		} else {
			return false;
		}
	};

	const removeUser = () => {
		window.localStorage.clear();
		setUser(default_user());
	};

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

				{/* Perf Routes */}
				<Route path="/perf">
					<PerfRoutes user={user} />
				</Route>

				{/* Pricing Routes */}
				<Route path="/pricing" element={<PricingPage user={user} />} />

				{/* Legal Routes */}
				<Route path="/legal">
					<LegalRoutes />
				</Route>

				{/* GitHub repo shortcut */}
				<Route path="/repo" element={<Repo />} />

				{/* Calendly demo shortcut */}
				<Route path="/demo" element={<Demo />} />
			</Routes>

			<For each={[...Array(16).keys()]}>{(_k, _i) => <br />}</For>
			<Footer />
		</>
	);
};

export default App;
