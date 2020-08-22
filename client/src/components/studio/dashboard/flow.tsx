import React from "react"
import { Link } from "@reach/router"

import { Columns } from "react-bulma-components"

import getFlow from "../utils/getFlow"

const Flow = (props: { id: string }) => {
  const flow = getFlow(props.id)
  return (
    <p>
      <Link to={"/studio/flow/#" + flow?.id?.toLowerCase()}>
        {flow?.name} Flow
      </Link>
    </p>
  )
}

export default Flow
