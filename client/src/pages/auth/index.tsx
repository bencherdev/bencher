// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Router, Link } from "@reach/router"

import SitePage from "../../components/site/pages/sitepage"

import Auth from "../../components/auth/auth"
import Logout from "../../components/auth/logout"

const AuthPage = () => (
  <SitePage link={Link}>
    <Router basepath="/auth">
      <Auth path="/auth/signup" />
      <Auth path="/auth/login" />
      <Logout path="/auth/logout" />
    </Router>
  </SitePage>
)

export default AuthPage
