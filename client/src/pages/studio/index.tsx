// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

const Studio = Loadable(() =>
  import("../../components/studio/workspace/studio")
)

const StudioPage = () => <Studio />

export default StudioPage
