// Base64 of text in any language, in a module of its own, which a page that
// needs no more of `convert` loads alone.

// https://developer.mozilla.org/en-US/docs/Glossary/Base64#the_unicode_problem
export const base64ToBytes = (base64) => {
	const binString = atob(base64);
	return Uint8Array.from(binString, (m) => m.codePointAt(0));
};

export const decodeBase64 = (base64) =>
	new TextDecoder().decode(base64ToBytes(base64));

export const bytesToBase64 = (bytes) => {
	const binString = String.fromCodePoint(...bytes);
	return btoa(binString);
};

export const encodeBase64 = (str) =>
	bytesToBase64(new TextEncoder().encode(str));
