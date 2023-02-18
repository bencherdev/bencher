import { useNavigate } from "solid-app-router";
import { createEffect } from "solid-js";
import { notification_path } from "../site/Notification";
import { NotifyKind, pageTitle } from "../site/util";

const AuthLogoutPage = (props: { config: any; removeUser: Function }) => {
	const navigate = useNavigate();

	props.removeUser();
	navigate(
		notification_path(
			props.config?.redirect,
			[],
			[],
			NotifyKind.ALERT,
			"Goodbye!",
		),
	);

	createEffect(() => {
		pageTitle("Logout");
	});

	return <></>;
};

export default AuthLogoutPage;
