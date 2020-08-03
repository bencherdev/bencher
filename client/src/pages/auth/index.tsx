// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Router } from "@reach/router"

import Auth from "../../components/auth/auth"
import Logout from "../../components/auth/logout"

const AuthPage = () => (
  <Router basepath="/auth">
    <Auth path="/auth/signup" />
    <Auth path="/auth/login" />
    <Logout path="/auth/logout" />
    <Auth path="/" />
  </Router>
)

export default AuthPage
