const authForms = {
    signup: {
        heading: "Sign up",
        form: {
            token: true,
            redirect: "/dashboard",
            notification: {
                success: "Sign up successful",
                danger: "Sign up failed"
            }
        }
    },
    login: {
        heading: "Log in",
        form: {
            token: true,
            redirect: "/dashboard",
            notification: {
                success: "Log in successful",
                danger: "Log in failed"
            }
        }
    },
};

export default authForms;