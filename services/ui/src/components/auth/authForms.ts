export const authForms = {
    signup: {
        title: "Sign up - Bencher",
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
        title: "Log in - Bencher",
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
