import React from "react"

const New = (props: { path: string }) => {
  let date = Date()
  return (
    <p>
      TODO create new {props.path} {window.location.href} at {date}
    </p>
  )
}

export default New
