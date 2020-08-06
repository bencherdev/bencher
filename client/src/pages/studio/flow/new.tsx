// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

const New = Loadable(() => import("../../../components/studio/flow/new"))

const NewFlowPage = () => <New />

export default NewFlowPage
