import React from "react"

const NewElement = (props: {
  position: { x: number; y: number }
  dimensions: { radius: number }
}) => {
  let radius = props?.dimensions?.radius
  let outer = props?.dimensions?.radius?.toString()
  return (
    <g fill="white" stroke="black" strokeWidth="5">
      <circle
        cx={props?.position?.x?.toString()}
        cy={props?.position?.y?.toString()}
        r={props?.dimensions?.radius?.toString()}
      />
      <circle
        cx={props?.position?.x?.toString()}
        cy={props?.position?.y?.toString()}
        r={(props?.dimensions?.radius / 2).toString()}
      />
    </g>
  )
}

export default NewElement
