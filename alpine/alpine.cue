// Base package for Alpine Linux
package alpine

import (
	"universe.dagger.io/docker"
)

// Build an Alpine Linux container image
#Build: {

	// Alpine version to install.
	version: string | *"1.60.0-alpine"

	// List of packages to install
	packages: [pkgName=string]: {
		// NOTE(samalba, gh issue #1532):
		//   it's not recommended to pin the version as it is already pinned by the major Alpine version
		//   version pinning is for future use (as soon as we support custom repositories like `community`,
		//   `testing` or `edge`)
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
						name: "apk"
						args: ["add", "\(pkgName)\(pkg.version)"]
						flags: {
							"-U":         true
							"--no-cache": true
						}
					}
				}
			},
		]
	}
}
