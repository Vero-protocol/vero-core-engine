import { Keypair, Networks, WebAuth } from "@stellar/stellar-sdk";
import { WalletConnector } from "../wallet-connector";

describe("WalletConnector", () => {
  const networkPassphrase = Networks.TESTNET;
  const domain = "test.vero.io";
  const serverKeypair = Keypair.random();
  const clientKeypair = Keypair.random();

  it("creates and verifies a valid challenge-response", () => {
    const xdr = WalletConnector.createChallenge({
      serverKeypair,
      clientAddress: clientKeypair.publicKey(),
      networkPassphrase,
      domain,
    });

    expect(xdr).toBeDefined();

    // Sign the challenge as the client
    const { tx: transaction } = WebAuth.readChallengeTx(
      xdr,
      serverKeypair.publicKey(),
      networkPassphrase,
      domain,
      domain
    );

    transaction.sign(clientKeypair);
    const signedXdr = transaction.toEnvelope().toXDR("base64").toString();

    const verifiedAddress = WalletConnector.verifyResponse(
      signedXdr,
      serverKeypair.publicKey(),
      networkPassphrase,
      domain
    );

    expect(verifiedAddress).toBe(clientKeypair.publicKey());
  });

  it("throws error for invalid signature", () => {
    const xdr = WalletConnector.createChallenge({
      serverKeypair,
      clientAddress: clientKeypair.publicKey(),
      networkPassphrase,
      domain,
    });

    // Don't sign as the client (or sign with wrong key)
    const otherKeypair = Keypair.random();
    const { tx: transaction } = WebAuth.readChallengeTx(
      xdr,
      serverKeypair.publicKey(),
      networkPassphrase,
      domain,
      domain
    );

    transaction.sign(otherKeypair);
    const signedXdr = transaction.toEnvelope().toXDR("base64").toString();

    expect(() => {
      WalletConnector.verifyResponse(
        signedXdr,
        serverKeypair.publicKey(),
        networkPassphrase,
        domain
      );
    }).toThrow("Invalid signature: client signature missing or incorrect");
  });
});
