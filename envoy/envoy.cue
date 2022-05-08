// Base package for Rust
package envoy

import (
	"universe.dagger.io/docker"
)

// Build a Debian Envoy container image
#Build: {

	// Alpine version to install.
	version: string | *"v1.21.2"

	docker.#Build & {
		steps: [
			docker.#Pull & {
				source: "index.docker.io/envoyproxy/envoy:\(version)"
			},
		]
	}
}
