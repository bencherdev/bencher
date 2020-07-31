import React from "react"

const Input = (props: {
  position: { x: number; y: number }
  dimensions: { radius: number }
}) => {
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

export default Input
