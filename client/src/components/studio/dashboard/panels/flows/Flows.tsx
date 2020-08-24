import React from "react"

import Flow from "./Flow"
const Flows = (props: { flows: any }) => {
  return (
    <React.Fragment>
      {props?.flows?.map((id: any) => {
        return <Flow key={id} id={id} />
      })}
    </React.Fragment>
  )
}

export default Flows
