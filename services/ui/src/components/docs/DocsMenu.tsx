import { Link } from "solid-app-router";
import { docs, getHref } from "./config/docs";
import { For } from "solid-js";

const DocsMenu = (_props) => {
	return (
		<aside class="menu">
			<For each={docs}>
				{(doc) => (
					<>
						<p class="menu-label">{doc.section?.name}</p>
						<ul class="menu-list">
							<For each={doc.pages}>
								{(page) => (
									<li>
										<Link href={getHref(doc.section?.slug, page.slug)}>
											{page.title}
										</Link>
									</li>
								)}
							</For>
						</ul>
					</>
				)}
			</For>
		</aside>
	);
};

export default DocsMenu;
