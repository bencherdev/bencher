import { authUser } from "../../util/auth";
import ThemeWordmark from "./theme/ThemeWordmark";

const AuthWordmark = () => {
	return (
		<a
			class="navbar-item"
			title="Bencher - Continuous Benchmarking"
			href={authUser()?.token ? "/console" : "/"}
		>
			<ThemeWordmark />
		</a>
	);
};

export default AuthWordmark;
