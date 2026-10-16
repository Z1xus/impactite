type Options = Record<string, unknown> & { challenge: string };

function bytes(text: string) {
	const base = text.replace(/-/g, "+").replace(/_/g, "/");
	const binary = atob(base + "=".repeat((4 - (base.length % 4)) % 4));
	return Uint8Array.from(binary, (char) => char.charCodeAt(0));
}

function text(buffer: ArrayBuffer | null) {
	if (!buffer) return null;
	let binary = "";
	for (const byte of new Uint8Array(buffer)) binary += String.fromCharCode(byte);
	return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function explain(error: unknown): never {
	if (error instanceof DOMException && error.name === "NotAllowedError")
		throw new Error("Passkey request was cancelled.");
	if (error instanceof DOMException && error.name === "InvalidStateError")
		throw new Error("This passkey is already saved.");
	// the server's address and this page's don't match, passkeys belong to one site
	if (error instanceof DOMException && error.name === "SecurityError")
		throw new Error("Passkeys only work at the address this server is set up for.");
	throw error instanceof Error ? error : new Error("Passkeys are not available here.");
}

export async function create(options: Options) {
	const user = options.user as { id: string };
	const exclude = (options.excludeCredentials as { id: string }[]) ?? [];
	const credential = (await navigator.credentials
		.create({
			publicKey: {
				...(options as unknown as PublicKeyCredentialCreationOptions),
				challenge: bytes(options.challenge),
				user: { ...(options.user as PublicKeyCredentialUserEntity), id: bytes(user.id) },
				excludeCredentials: exclude.map((item) => ({
					type: "public-key",
					id: bytes(item.id)
				}))
			}
		})
		.catch(explain)) as PublicKeyCredential;
	const response = credential.response as AuthenticatorAttestationResponse;
	return {
		id: credential.id,
		rawId: text(credential.rawId),
		type: credential.type,
		response: {
			clientDataJSON: text(response.clientDataJSON),
			attestationObject: text(response.attestationObject)
		}
	};
}

export async function get(options: Options) {
	const credential = (await navigator.credentials
		.get({
			publicKey: {
				...(options as unknown as PublicKeyCredentialRequestOptions),
				challenge: bytes(options.challenge),
				allowCredentials: []
			}
		})
		.catch(explain)) as PublicKeyCredential;
	const response = credential.response as AuthenticatorAssertionResponse;
	return {
		id: credential.id,
		rawId: text(credential.rawId),
		type: credential.type,
		response: {
			clientDataJSON: text(response.clientDataJSON),
			authenticatorData: text(response.authenticatorData),
			signature: text(response.signature),
			userHandle: text(response.userHandle)
		}
	};
}
