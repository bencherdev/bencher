const CreateToken = (props: { user_slug?: string }) => {
	const link = props.user_slug
		? `/console/users/${props.user_slug}/tokens/add`
		: "/auth/signup";
	return (
		<a href={link} target="_blank">
			Add an API Token
		</a>
	);
};

export default CreateToken;
