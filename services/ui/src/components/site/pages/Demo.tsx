import HeaderPage from "./HeaderPage";
import { BENCHER_CALENDLY_URL } from "../util";

const Demo = () => {
	window.location.href = BENCHER_CALENDLY_URL;

	return (
		<HeaderPage
			page={{
				title: "Calendly Demo Redirect - Bencher",
				header: "Redirecting...",
				content: (
					<p>
						Redirecting to <a href={BENCHER_CALENDLY_URL}>Calendly</a>...
					</p>
				),
			}}
		/>
	);
};

export default Demo;
