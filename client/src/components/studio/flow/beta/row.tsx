import React from "react"
import * as d3 from "d3"

class Row extends React.Component {
  state = {
    open: false,
    y: this.props.y,
  }

  rowRef = React.createRef()

  toggleOpen = () => this.setState({ open: !this.state.open })

  contentRefCallback = element => {
    if (element) {
      this.contentRef = element
      this.props.reportHeight(element.getBoundingClientRect().height)
    } else {
      this.props.reportHeight()
    }
  }

  contentUpdated = () => {
    this.props.reportHeight(this.contentRef.getBoundingClientRect().height)
  }

  componentDidUpdate() {
    const { y } = this.props
    d3.select(this.rowRef.current)
      .transition()
      .duration(500)
      .ease(d3.easeCubicInOut)
      .attr("transform", `translate(5, ${y})`)
      .on("end", () => {
        this.setState({
          y,
        })
      })
  }

  render() {
    const { icon, content, width, height } = this.props,
      { y } = this.state

    return (
      <g transform={`translate(5, ${y})`} ref={this.rowRef}>
        <g onClick={this.toggleOpen} style={{ cursor: "pointer" }}>
          {icon}
        </g>
        {this.state.open ? (
          <g>
            <foreignObject
              x={20}
              y={-20}
              width={width}
              height={height}
              style={{ border: "1px solid red" }}
            >
              <div ref={this.contentRefCallback}>
                {typeof content === "function"
                  ? content(this.contentUpdated)
                  : content}
              </div>
            </foreignObject>
            <circle x={20} y={-20} r={5} />
          </g>
        ) : null}
      </g>
    )
  }
}

export default Row
