import { lazy } from "solid-js";
import { Route, Navigate } from "solid-app-router";

const AuthFormPage = lazy(() => import("./AuthFormPage"));
const AuthLogoutPage = lazy(() => import("./AuthLogoutPage"));
const AuthConfirmPage = lazy(() => import("./AuthConfirmPage"));

import { JsonConfirm } from "../../types/bencher";

const AuthRoutes = (props: {
	user: JsonConfirm;
	handleUser: (user: JsonConfirm) => boolean;
	removeUser: () => void;
}) => {
	return (
		<>
			<Route path="/" element={<Navigate href="/auth/signup" />} />
			<Route
				path="/signup"
				element={<AuthFormPage new_user={true} user={props.user} />}
			/>
			<Route
				path="/login"
				element={<AuthFormPage new_user={false} user={props.user} />}
			/>
			<Route
				path="/confirm"
				element={
					<AuthConfirmPage user={props.user} handleUser={props.handleUser} />
				}
			/>
			<Route
				path="/logout"
				element={<AuthLogoutPage removeUser={props.removeUser} />}
			/>
		</>
	);
};

export default AuthRoutes;
