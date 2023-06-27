import { Link } from "solid-app-router";
import { docs, getHref } from "../config/docs";
import { For } from "solid-js";

const DocsIndex = (_props) => {
	return (
		<div class="content">
			<ul>
				<For each={docs}>
					{(doc) => (
						<li>
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
						</li>
					)}
				</For>
			</ul>
			<br />
		</div>
	);
};

export default DocsIndex;
