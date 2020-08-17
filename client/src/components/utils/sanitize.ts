// http://shebang.mintern.net/foolproof-html-escaping-in-javascript/
// https://gist.github.com/towfiqpiash/d6b51e97120adbea5a4581edc6094219
function toHtml(text: string) {
  let div = document.createElement("div")
  div.appendChild(document.createTextNode(text))
  return div.innerHTML
}

function toText(html: string) {
  let div = document.createElement("div")
  div.innerHTML = html
  return div.innerText
}

export default { toHtml, toText }
