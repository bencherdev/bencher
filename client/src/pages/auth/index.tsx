// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

import SEO from "../../components/utils/seo"
const Auth = Loadable(() => import("../../components/auth/Auth"))

const AuthPage = () => (
  <React.Fragment>
    <SEO title="Authentication" />
    <Auth context="root" />{" "}
  </React.Fragment>
)

export default AuthPage
