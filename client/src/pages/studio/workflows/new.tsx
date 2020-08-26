// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

import SEO from "../../../components/utils/seo"
const New = Loadable(() => import("../../../components/studio/workflow/New"))

const NewWorkflowPage = () => (
  <React.Fragment>
    <SEO title="New Workflow" />
    <New />
  </React.Fragment>
)

export default NewWorkflowPage
