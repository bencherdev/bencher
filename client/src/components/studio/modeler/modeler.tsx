import React from "react"

const Modeler = (props: { path: string; id: string }) => {
  let date = Date()
  return (
    <p>
      Modeler {props.path} {props.id} {date}
    </p>
  )
}

export default Modeler
