// Base package for Rust
package rust

import (
	"universe.dagger.io/docker"
)

// Build a Debian Rust container image
#Build: {

	// Alpine version to install.
	version: string | *"1.60.0-buster"

	// List of packages to install
	packages: [pkgName=string]: {
		version: string | *""
	}

	docker.#Build & {
		steps: [
			docker.#Pull & {
				source: "index.docker.io/rust:\(version)"
			},
			for pkgName, pkg in packages {
				docker.#Run & {
					command: {
						name: "apt-get"
						args: ["install", "\(pkgName)\(pkg.version)"]
						flags: {
							"-y": true
						}
					}
				}
			},
		]
	}
}
