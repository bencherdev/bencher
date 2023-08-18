import { authUser } from "../../../util/auth";

const AddAnApiToken = () => (
	<a
		href={`/console/users/${authUser()?.user?.slug}/tokens/add`}
		target="_blank"
	>
		Add an API Token
	</a>
);

export default AddAnApiToken;
