import React, { useEffect, useState } from "react"

// TODO update the arrow so that the NewLine Element
// is the verticle alignment point for all arrows
// as well as for all elements' midpoints
const Arrow = (props: { source: any; destination: any }) => {
  const [path, setPath] = useState({
    source: {},
    destination: {},
  })
  const [arrow, setArrow] = useState({
    shaft: "",
    head: "",
  })

  function handleArrow() {
    setPath({ source: props.source, destination: props.destination })
    setArrow(getArrow())
  }

  function getArrow() {
    let start
    switch (props?.source?.type) {
      case "input":
      case "subflow":
        start = {
          x: props?.source?.position?.x + props?.source?.dimensions?.radius,
          y: props?.source?.position?.y,
        }
        break
      case "parent":
      case "table":
      case "decision":
      case "function":
      default:
        start = {
          x: props?.source?.position?.x + props?.source?.dimensions?.width,
          y: props?.source?.position?.y + props?.source?.dimensions?.height / 2,
        }
    }

    let end
    switch (props?.destination?.type) {
      case "input":
      case "subflow":
      case "return":
        end = {
          x:
            props?.destination?.position?.x -
            props?.destination?.dimensions?.radius,
          y: start.y,
        }
        break
      case "table":
      case "decision":
      case "function":
      default:
        end = { x: props?.destination?.position?.x, y: start.y }
    }

    return {
      shaft:
        positionString(start) +
        " " +
        positionString({ x: end?.x - 15, y: end?.y }),
      head:
        positionString(end) +
        " " +
        positionString({ x: end?.x - 25, y: end?.y + 15 }) +
        " " +
        positionString({ x: end?.x - 25, y: end?.y - 15 }),
    }
  }

  function positionString(position: { x: number; y: number }) {
    return position?.x + "," + position?.y
  }

  useEffect(() => {
    if (
      props?.source !== path.source ||
      props?.destination !== path.destination
    ) {
      handleArrow()
    }
  }, [])

  return (
    <g>
      <polyline points={arrow?.shaft} stroke="black" strokeWidth="5" />
      <polygon points={arrow?.head} stroke="black" fill="black" />
    </g>
  )
}

export default Arrow
