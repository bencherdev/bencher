// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

const App = Loadable(() => import("../../components/app/App"))

const AppPage = () => <App />

export default AppPage
