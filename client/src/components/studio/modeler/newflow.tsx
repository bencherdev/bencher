import React from "react"

const NewFlow = (props: { path: string }) => {
  let date = Date()
  return (
    <p>
      TODO create a new {props.path.substring(props.path.lastIndexOf("/") + 1)}{" "}
      {date}
    </p>
  )
}

export default NewFlow
