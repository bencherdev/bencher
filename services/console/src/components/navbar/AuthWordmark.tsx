import { authUser } from "../../util/auth";
import { BENCHER_WORDMARK_ID } from "../../util/ext";
import ThemeWordmark from "./theme/ThemeWordmark";

const AuthWordmark = () => {
	return (
		<a
			class="navbar-item"
			title="Bencher - Continuous Benchmarking"
			href={authUser()?.token ? "/console" : "/"}
		>
			<ThemeWordmark id={BENCHER_WORDMARK_ID} />
		</a>
	);
};

export default AuthWordmark;
