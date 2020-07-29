import React from "react"

const Arrow = (props: { source: any; destination: any }) => {
  let start = props?.source?.position?.x
  if (props?.source?.type === "new") {
    console.log(props.source?.dimensions?.radius)
    start += props.source?.dimensions?.radius / 2
  }
  start = start.toString()
  let end = props?.destination?.position?.x?.toString()
  let y = props?.source?.position?.y?.toString()
  console.log(start)
  console.log(end)
  return <line x1={start} y1={y} x2={end} y2={y} stroke="black" />
}

export default Arrow
