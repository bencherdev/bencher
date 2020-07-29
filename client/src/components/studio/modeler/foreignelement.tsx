import React from "react"

const ForeignElement = (props: {
  position: { x: number; y: number }
  dimensions: { width: number; height: number }
  children: any
}) => {
  return (
    <foreignObject
      x={props?.position?.x?.toString()}
      y={props?.position?.y?.toString()}
      width={props?.dimensions?.width?.toString()}
      height={props?.dimensions?.height?.toString()}
    >
      {props.children}
    </foreignObject>
  )
}

export default ForeignElement
