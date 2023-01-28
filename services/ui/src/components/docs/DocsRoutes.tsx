import { Route, Navigate } from "solid-app-router";
import DocsPage from "./DocsPage";
import { For } from "solid-js";
import { docs, getPath } from "./config/docs";

const DocsRoutes = (_props) => {
	return (
		<>
			{/* Docs Routes */}
			<Route
				path="/"
				element={<Navigate href="/docs/tutorial/quick-start" />}
			/>
			<Route path="/tutorial" element={<Navigate href="/docs" />} />
			<Route path="/how-to" element={<Navigate href="/docs" />} />
			{/* TODO remove in due time */}
			<Route
				path="/how-to/quick-start"
				element={<Navigate href="/docs/tutorial/quick-start" />}
			/>
			<Route
				path="/explanation/branch-management"
				element={<Navigate href="/docs/explanation/cli-branch-selection" />}
			/>
			<Route path="/explanation" element={<Navigate href="/docs" />} />
			<Route path="/reference" element={<Navigate href="/docs" />} />
			<For each={docs}>
				{(doc) => (
					<>
						<For each={doc.pages}>
							{(page) => (
								<Route
									path={getPath(doc.section?.slug, page.slug)}
									element={<DocsPage page={page} />}
								/>
							)}
						</For>
					</>
				)}
			</For>
		</>
	);
};

export default DocsRoutes;
