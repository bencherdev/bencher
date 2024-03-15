import Input from "../field/kinds/Input";
import { EMAIL } from "./authFields";

const LoginForm = () => {
	return (
		<form class="box">
			<div class="field">
				{<label class="label is-medium">Email</label>}
				<Input
					label="Email"
					value=""
					valid={null}
					config={EMAIL}
					handleField={() => {}}
				/>
			</div>
			<br />
			<div class="field">
				<p class="control">
					<button
						class="button is-primary is-fullwidth"
						type="submit"
						disabled={true}
						onClick={(e) => {
							e.preventDefault();
						}}
					>
						Log in
					</button>
				</p>
			</div>
		</form>
	);
};
export default LoginForm;
