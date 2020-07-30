import React, { useEffect, useState } from "react"

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
      case "new":
      case "subflow":
        start = {
          x: props?.source?.position?.x + props?.source?.dimensions?.radius,
          y: 0,
        }
        break
      case "parent":
      case "table":
      case "decision":
      case "function":
      default:
        start = {
          x: props?.source?.position?.x + props?.source?.dimensions?.width,
          y: 0,
        }
    }

    let end
    switch (props?.destination?.type) {
      case "subflow":
      case "return":
        end = {
          x:
            props?.destination?.position?.x -
            props?.destination?.dimensions?.radius,
          y: 0,
        }
        break
      case "table":
      case "decision":
      case "function":
      default:
        end = { x: props?.destination?.position?.x, y: 0 }
    }

    let y = props?.source?.position?.y
    start.y = y
    end.y = y
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
