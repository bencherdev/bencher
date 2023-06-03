import { useNavigate } from "solid-app-router";
import { createEffect } from "solid-js";
import { notification_path } from "../site/Notification";
import { NotifyKind, pageTitle } from "../site/util";

const AuthLogoutPage = (props: { removeUser: () => void }) => {
	const navigate = useNavigate();

	createEffect(() => {
		pageTitle("Log out");

		props.removeUser();

		navigate(
			notification_path("/auth/login", [], [], NotifyKind.ALERT, "Goodbye!"),
		);
	});

	return <></>;
};

export default AuthLogoutPage;
