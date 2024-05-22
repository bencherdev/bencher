import { authUser } from "../../../util/auth";

const AddAnApiToken = (props: { text: string }) => (
	<a
		href={`/console/users/${authUser()?.user?.slug}/tokens/add`}
		target="_blank"
		rel="noreferrer"
	>
		{props.text}
	</a>
);

export default AddAnApiToken;
