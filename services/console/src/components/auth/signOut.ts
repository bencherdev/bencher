import { deleteCache } from "../../next/idb";
import { forgetReader } from "../../next/memory";

/** Forget the signed in user, and everything the console keeps of them in this browser. */
export const signOut = () => {
	forgetReader(localStorage);
	deleteCache();
};
