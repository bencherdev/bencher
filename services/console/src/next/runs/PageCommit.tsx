import Skeleton from "@bencherdev/ui/Skeleton";
import { Match, Switch } from "solid-js";
import CommitLine from "../commit";
import type { CommitRow, GitHubLinks } from "../github";

const NO_COMMIT: CommitRow = { kind: "muted", text: "No commit" };

/** A run's or a report's commit row, between its title and its facts; a placeholder holds its line until the run or report and the project's bootstrap arrive, and a bootstrap that never arrives leaves the line empty. */
const PageCommit = (props: {
	/** The run's or the report's hash and title, undefined while it loads. */
	commit: { hash?: string | undefined; title?: string | undefined } | undefined;
	/** The project's bootstrap is on its way. */
	pending: boolean;
	github: GitHubLinks;
}) => {
	const row = () => {
		const commit = props.commit;
		if (commit === undefined) {
			return undefined;
		}
		return commit.hash === undefined
			? NO_COMMIT
			: props.github.row(commit.hash, commit.title);
	};
	return (
		<Switch fallback={<div class="rp-commit" />}>
			<Match when={props.commit === undefined || props.pending}>
				<div class="rp-commit">
					<Skeleton size="text" class="rp-commit-skel" />
				</div>
			</Match>
			<Match when={row()}>
				{(shown) => (
					<div class="rp-commit">
						<CommitLine row={shown()} links />
					</div>
				)}
			</Match>
		</Switch>
	);
};

export default PageCommit;
