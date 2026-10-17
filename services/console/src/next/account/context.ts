import { createContext, useContext } from "solid-js";
import type { JsonAuthUser, JsonUser } from "../../types/bencher";
import type { Api } from "../api";

interface Account {
	api: Api;
	reader: () => JsonAuthUser;
	/** The reader's own account as the API now has it. */
	keepUser: (user: JsonUser) => void;
	/** End the session and sign in again, back where the reader is. */
	signOut: () => void;
}

/** The API and the signed in reader, for the account pages. */
export const AccountContext = createContext<Account>();

export const useAccount = () => {
	const account = useContext(AccountContext);
	if (!account) {
		throw new Error("useAccount outside of the console's account pages");
	}
	return account;
};
