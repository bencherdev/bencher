import { deleteCache } from "../../next/idb";
import { forgetShell } from "../../next/memory";
import { removeUser } from "../../util/auth";
import { removeOrganization } from "../../util/organization";

/** Forget the signed in user, and everything the console keeps of them in this browser. */
export const signOut = () => {
	removeUser();
	removeOrganization();
	// What the new console remembers of this reader goes with them.
	forgetShell(localStorage);
	deleteCache();
};
