import("../crate/pkg")
  .then(leedor => leedor.run())
  .catch(e => console.error("Error on import", e))
