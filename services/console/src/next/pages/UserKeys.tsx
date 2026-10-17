import { useParams } from "@solidjs/router";
import { useAccount } from "../account/context";
import Head from "../account/Head";
import { userKeys } from "../account/keys";
import { Frame } from "./Profile";
import { KeyList } from "./Settings";

/** The reader's user keys, as a project's Keys page lists its own. */
const UserKeys = () => {
	const { api } = useAccount();
	const params = useParams();
	const user = () => params.user ?? "";
	return (
		<Frame user={user()} section="keys">
			<KeyList
				api={api}
				home={userKeys(user())}
				manage
				consequence="Anything using it stops working, for good."
				head={(actions) => <Head title="Keys" actions={actions} />}
			/>
		</Frame>
	);
};

export default UserKeys;

// Legacy tokens loads this chunk rather than split a shared one.
export { default as Head } from "../account/Head";
export { legacyTokenList } from "../account/keys";
