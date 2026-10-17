import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import Head from "./Head";
import { projectKeys } from "./data";
import KeyList from "./KeyList";

/** The project keys runs report with: listed, made, shown once, and revoked. */
const Keys = () => {
	const { api, slug } = useProject();
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	// Only a Maintainer may list keys.
	const manage = () => bootstrap().data?.permissions.manage;
	return (
		<KeyList
			api={api}
			home={projectKeys(slug())}
			manage={manage()}
			consequence="Every run that uses it stops reporting, for good."
			head={(actions) => (
				<Head
					slug={slug()}
					title="Keys"
					readOnly={manage() === false}
					actions={actions}
				/>
			)}
		/>
	);
};

export default Keys;
