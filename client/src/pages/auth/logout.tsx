// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

const Logout = Loadable(() => import("../../components/auth/logout"))

const LogoutPage = () => <Logout />

export default LogoutPage
