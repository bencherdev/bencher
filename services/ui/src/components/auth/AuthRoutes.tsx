import { lazy } from "solid-js";
import { Route, Navigate } from "solid-app-router";

const AuthFormPage = lazy(() => import("./AuthFormPage"));
const AuthLogoutPage = lazy(() => import("./AuthLogoutPage"));
const AuthConfirmPage = lazy(() => import("./AuthConfirmPage"));

import AUTH_CONFIG from "./config/auth";
import { Auth } from "./config/types";
import { UiUser } from "../../App";

const AuthRoutes = (props: {
	user: UiUser;
	handleUser: Function;
	removeUser: Function;
}) => {
	return (
		<>
			<Route path="/" element={<Navigate href="/auth/signup" />} />
			<Route
				path="/signup"
				element={
					<AuthFormPage
						new_user={true}
						user={props.user}
						handleUser={props.handleUser}
					/>
				}
			/>
			<Route
				path="/login"
				element={
					<AuthFormPage
						new_user={false}
						user={props.user}
						handleUser={props.handleUser}
					/>
				}
			/>
			<Route
				path="/confirm"
				element={
					<AuthConfirmPage
						config={AUTH_CONFIG[Auth.CONFIRM]}
						user={props.user}
						handleUser={props.handleUser}
					/>
				}
			/>
			<Route
				path="/logout"
				element={
					<AuthLogoutPage
						config={AUTH_CONFIG[Auth.LOGOUT]}
						removeUser={props.removeUser}
					/>
				}
			/>
		</>
	);
};

export default AuthRoutes;
