import { Keypair, Networks, WebAuth } from "@stellar/stellar-sdk";
import { WalletConnector, type AccountLoader } from "../wallet-connector";

describe("WalletConnector", () => {
  const networkPassphrase = Networks.TESTNET;
  const domain = "test.vero.io";
  const serverKeypair = Keypair.random();
  const clientKeypair = Keypair.random();

  /** A single master key at full weight, med_threshold 1 — the common case. */
  const masterKeyLoader = (accountId: string): AccountLoader =>
    async () => ({
      signers: [{ key: accountId, weight: 1, type: "ed25519_public_key" }],
      thresholds: { med_threshold: 1 },
    });

  function signedChallenge(signer: Keypair): string {
    const xdr = WalletConnector.createChallenge({
      serverKeypair,
      clientAddress: clientKeypair.publicKey(),
      networkPassphrase,
      domain,
    });

    const { tx: transaction } = WebAuth.readChallengeTx(
      xdr,
      serverKeypair.publicKey(),
      networkPassphrase,
      domain,
      domain
    );

    transaction.sign(signer);
    return transaction.toEnvelope().toXDR("base64").toString();
  }

  it("creates and verifies a valid challenge-response", async () => {
    const verifiedAddress = await WalletConnector.verifyResponse(
      signedChallenge(clientKeypair),
      serverKeypair.publicKey(),
      networkPassphrase,
      domain,
      masterKeyLoader(clientKeypair.publicKey())
    );

    expect(verifiedAddress).toBe(clientKeypair.publicKey());
  });

  it("throws error for invalid signature", async () => {
    const otherKeypair = Keypair.random();

    await expect(
      WalletConnector.verifyResponse(
        signedChallenge(otherKeypair),
        serverKeypair.publicKey(),
        networkPassphrase,
        domain,
        masterKeyLoader(clientKeypair.publicKey())
      )
    ).rejects.toThrow("Invalid signature: client signature missing or incorrect");
  });

  // Verification used to pass a fabricated signer set — the account ID read out
  // of the submitted XDR, at weight 1, threshold 1 — and never fetched the real
  // account. A revoked master key (weight 0) therefore still authenticated.
  it("rejects a master key whose weight has been revoked to zero", async () => {
    const revokedLoader: AccountLoader = async () => ({
      signers: [
        { key: clientKeypair.publicKey(), weight: 0, type: "ed25519_public_key" },
      ],
      thresholds: { med_threshold: 1 },
    });

    await expect(
      WalletConnector.verifyResponse(
        signedChallenge(clientKeypair),
        serverKeypair.publicKey(),
        networkPassphrase,
        domain,
        revokedLoader
      )
    ).rejects.toThrow("Invalid signature: client signature missing or incorrect");
  });

  // A single signature must not satisfy a multi-sig account: the old fabricated
  // set hardcoded threshold 1 regardless of the account's real med_threshold.
  it("rejects a single signature on a multi-sig account below threshold", async () => {
    const coSigner = Keypair.random();
    const multisigLoader: AccountLoader = async () => ({
      signers: [
        { key: clientKeypair.publicKey(), weight: 1, type: "ed25519_public_key" },
        { key: coSigner.publicKey(), weight: 1, type: "ed25519_public_key" },
      ],
      thresholds: { med_threshold: 2 },
    });

    await expect(
      WalletConnector.verifyResponse(
        signedChallenge(clientKeypair),
        serverKeypair.publicKey(),
        networkPassphrase,
        domain,
        multisigLoader
      )
    ).rejects.toThrow("Invalid signature: client signature missing or incorrect");
  });

  it("fails closed when the account cannot be loaded", async () => {
    const failingLoader: AccountLoader = async () => {
      throw new Error("404 Not Found");
    };

    await expect(
      WalletConnector.verifyResponse(
        signedChallenge(clientKeypair),
        serverKeypair.publicKey(),
        networkPassphrase,
        domain,
        failingLoader
      )
    ).rejects.toThrow(/unable to load account/i);
  });
});
