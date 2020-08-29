// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

import SEO from "../../components/utils/seo"
const Logout = Loadable(() => import("../../components/auth/Logout"))

const LogoutPage = () => (
  <React.Fragment>
    <SEO title="Log out" />
    <Logout />
  </React.Fragment>
)

export default LogoutPage
