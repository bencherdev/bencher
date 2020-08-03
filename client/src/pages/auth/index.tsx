// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Router } from "@reach/router"

import Auth from "../../components/auth/auth"
import Logout from "../../components/auth/logout"

const AuthPage = () => (
  <Router basepath="/auth">
    <Auth path="/signup" context="signup" />
    <Auth path="/login" context="login" />
    <Logout path="/logout" />
    <Auth path="/" context="root" />
  </Router>
)

export default AuthPage
