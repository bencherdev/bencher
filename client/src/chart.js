export function show_chart(ticks) {
  var margin = {
      top: 10,
      right: 30,
      bottom: 30,
      left: 60,
    },
    width = 1200 - margin.left - margin.right,
    height = 600 - margin.top - margin.bottom;

  var svg = d3
    .select("#chart")
    .append("svg")
    .attr("width", width + margin.left + margin.right)
    .attr("height", height + margin.top + margin.bottom)
    .append("g")
    .attr("transform", "translate(" + margin.left + "," + margin.top + ")");

  var x = d3
    .scaleTime()
    .domain(
      d3.extent(ticks, function (d) {
        return d.time * 1000;
      })
    )
    .nice()
    .range([0, width]);

  svg
    .append("g")
    .attr("transform", "translate(0," + height + ")")
    .call(d3.axisBottom(x).tickFormat(d3.utcFormat("%y/%m/%d %H:%M")));

  var y = d3
    .scaleLinear()
    .domain([d3.min(ticks, (d) => d.bottom), d3.max(ticks, (d) => d.top)])
    .nice()
    .range([height, 0]);

  svg.append("g").call(d3.axisLeft(y));

  const g = svg
    .append("g")
    .attr("stroke-linecap", "round")
    .attr("stroke", "black")
    .selectAll("g")
    .data(ticks)
    .join("g")
    .attr("transform", (d) => `translate(${x(d.time * 1000)},0)`);

  g.append("line")
    .attr("y1", (d) => y(d.bottom))
    .attr("y2", (d) => y(d.top));

  g.append("line")
    .attr("y1", (d) => y(d.open))
    .attr("y2", (d) => y(d.close))
    .attr("stroke-width", 4)
    .attr("stroke", (d) =>
      d.open > d.close ? "red" : d.close > d.open ? "green" : "black"
    );
}
