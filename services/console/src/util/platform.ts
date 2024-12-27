export enum Platform {
	Unix = "unix",
	Windows = "windows",
}

// https://github.com/rust-lang/rustup/blob/7ccf717e6e1aee46f65cc6fea4132a3f0e37593b/www/rustup.js
export const getPlatform = async () => {
	switch (navigator.platform) {
		case "Win32":
		case "Win64":
			return Platform.Windows;
		case "Linux x86_64":
		case "Linux i686":
		case "Linux i686 on x86_64":
		case "Linux aarch64":
		case "Linux armv6l":
		case "Linux armv7l":
		case "Linux armv8l":
		case "Linux ppc64":
		case "Linux mips":
		case "Linux mips64":
		case "Linux riscv64":
		case "Mac":
		case "FreeBSD x86_64":
		case "FreeBSD amd64":
		case "NetBSD x86_64":
		case "NetBSD amd64":
		case "SunOS i86pc":
			return Platform.Unix;
		default:
			if (
				(navigator.appVersion?.indexOf("Win") ?? -1) != -1 ||
				(navigator.appVersion?.indexOf("Mac") ?? -1) != -1 ||
				(navigator.appVersion?.indexOf("FreeBSD") ?? -1) != -1
			) {
				return Platform.Unix;
			}
			if (
				(navigator.oscpu?.indexOf("Win32") ?? -1) != -1 ||
				(navigator.oscpu?.indexOf("Win64") ?? -1) != -1
			) {
				return Platform.Windows;
			}
			if (
				(navigator.oscpu?.indexOf("Mac") ?? -1) != -1 ||
				(navigator.oscpu?.indexOf("Linux") ?? -1) != -1 ||
				(navigator.oscpu?.indexOf("FreeBSD") ?? -1) != -1 ||
				(navigator.oscpu?.indexOf("NetBSD") ?? -1) != -1 ||
				(navigator.oscpu?.indexOf("SunOS") ?? -1) != -1
			) {
				return Platform.Unix;
			}
			// Default to Unix if unknown
			return Platform.Unix;
	}
};
