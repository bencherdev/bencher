import("../pkg/index.js")
  .then((module) => {
    module.main_js();
  })
  .catch(console.error);
