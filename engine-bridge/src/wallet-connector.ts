import {
  Keypair,
  WebAuth,
} from "@stellar/stellar-sdk";

export interface ChallengeOptions {
  serverKeypair: Keypair;
  clientAddress: string;
  networkPassphrase: string;
  domain: string;
  timeout?: number;
}

export class WalletConnector {
  /**
   * Generates a SEP-10 challenge transaction XDR.
   */
  static createChallenge(options: ChallengeOptions): string {
    const { serverKeypair, clientAddress, networkPassphrase, domain, timeout = 300 } = options;

    return WebAuth.buildChallengeTx(
      serverKeypair,
      clientAddress,
      domain,
      timeout,
      networkPassphrase,
      domain // webAuthDomain defaults to homeDomain if not specified
    );
  }

  /**
   * Verifies a signed SEP-10 challenge transaction.
   */
  static verifyResponse(
    xdr: string,
    serverAddress: string,
    networkPassphrase: string,
    domain: string
  ): string {
    const { clientAccountID } = WebAuth.readChallengeTx(
      xdr,
      serverAddress,
      networkPassphrase,
      domain,
      domain // webAuthDomain
    );

    let signersFound: string[];
    try {
      signersFound = WebAuth.verifyChallengeTxThreshold(
        xdr,
        serverAddress,
        networkPassphrase,
        1, // threshold
        [{ key: clientAccountID, weight: 1, type: "ed25519_public_key" }],
        domain,
        domain // webAuthDomain
      );
    } catch (err) {
      throw new Error("Invalid signature: client signature missing or incorrect");
    }

    if (signersFound.length === 0) {
      throw new Error("Invalid signature: client signature missing or incorrect");
    }

    return clientAccountID;
  }
}
