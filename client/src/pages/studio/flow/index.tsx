// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

const Notebook = Loadable(() =>
  import("../../../components/studio/notebook/Notebook")
)

const NotebookPage = () => <Notebook />

export default NotebookPage
