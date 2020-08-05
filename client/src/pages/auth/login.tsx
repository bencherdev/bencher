// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

const Auth = Loadable(() => import("../../components/auth/auth"))

const LoginPage = () => <Auth context="login" />

export default LoginPage
