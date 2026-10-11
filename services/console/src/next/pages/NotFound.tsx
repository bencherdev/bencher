import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import { Show } from "solid-js";
import ContentSkeleton from "../ContentSkeleton";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";

/** A path in the project that no page draws. */
const NotFound = () => {
	const { api, slug } = useProject();
	const project = useQueryResult(() => consoleProjectQuery(api, slug()));
	return (
		<Show when={project().data} fallback={<ContentSkeleton />}>
			{(data) => (
				<main class="page">
					<div class="pagehead">
						<Heading level={1} size="xl">
							Page not found
						</Heading>
						<Show when={!data().permissions.edit}>
							<p class="ro-note">
								<Icon name="read-only" />
								Read only. Ask a project Maintainer for access.
							</p>
						</Show>
					</div>
				</main>
			)}
		</Show>
	);
};

export default NotFound;
