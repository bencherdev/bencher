import HeaderPage from "./HeaderPage";

const RedirectPage = (props) => {
	window.location.href = props.url;

	return (
		<HeaderPage
			page={{
				title: `Bencher ${props.title} Redirect`,
				heading: "Redirecting",
				content: (
					<p>
						Redirecting to <a href={props.url}>{props.title}</a>...
					</p>
				),
			}}
		/>
	);
};

export default RedirectPage;
