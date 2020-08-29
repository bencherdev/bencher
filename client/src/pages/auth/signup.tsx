// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

import SEO from "../../components/utils/seo"
const Auth = Loadable(() => import("../../components/auth/Auth"))

const SignupPage = () => (
  <React.Fragment>
    <SEO title="Sign up" />
    <Auth context="signup" />
  </React.Fragment>
)

export default SignupPage
