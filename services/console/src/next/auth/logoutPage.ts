import { deleteCache } from "../idb";
import { forgetReader } from "../memory";
import { endSession } from "../session";

/** Sign out: end the session, forget the reader and everything kept of them, then the door. */
const signOut = async () => {
	await endSession();
	forgetReader(localStorage);
	deleteCache();
	location.replace("/auth/login");
};

signOut();
