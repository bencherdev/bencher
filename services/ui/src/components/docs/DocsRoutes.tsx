import { Route, Navigate } from "solid-app-router";
import DocsPage from "./DocsPage";
import { For } from "solid-js";
import { docs, getPath } from "./config/docs";
import DocsIndex from "./pages/DocsIndex";
import PageKind from "./config/page_kind";

const DocsRoutes = (_props) => {
	return (
		<>
			{/* Docs Routes */}
			<Route
				path="/"
				element={
					<DocsPage
						page={{
							title: "Bencher Docs",
							panel: {
								kind: PageKind.DIRECTORY,
								heading: "Bencher Docs",
								content: <DocsIndex />,
							},
						}}
					/>
				}
			/>
			<Route path="/tutorial" element={<Navigate href="/docs" />} />
			<Route path="/how-to" element={<Navigate href="/docs" />} />
			<Route path="/explanation" element={<Navigate href="/docs" />} />
			<Route path="/reference" element={<Navigate href="/docs" />} />
			{/* Historical route forwarding */}
			<Route
				path="/how-to/quick-start"
				element={<Navigate href="/docs/tutorial/quick-start" />}
			/>
			<Route
				path="/explanation/branch-management"
				element={<Navigate href="/docs/explanation/branch-selection" />}
			/>
			<Route
				path="/explanation/cli-branch-selection"
				element={<Navigate href="/docs/explanation/branch-selection" />}
			/>
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
