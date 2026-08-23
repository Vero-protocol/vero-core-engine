import {
  Horizon,
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

/** The subset of a Horizon account this module needs. Keeps it mockable in tests. */
export interface AccountSigners {
  signers: Array<{ key: string; weight: number; type: string }>;
  thresholds: { med_threshold: number };
}

/** Loads a client account's real signer configuration. */
export type AccountLoader = (accountId: string) => Promise<AccountSigners>;

/** Builds an `AccountLoader` backed by a real Horizon server. */
export function horizonAccountLoader(server: Horizon.Server): AccountLoader {
  return async (accountId: string) => {
    const account = await server.loadAccount(accountId);
    return {
      signers: account.signers.map((s) => ({
        key: s.key,
        weight: s.weight,
        type: s.type,
      })),
      thresholds: { med_threshold: account.thresholds.med_threshold },
    };
  };
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
   * Verifies a signed SEP-10 challenge transaction against the client
   * account's **real** signer configuration.
   *
   * This previously passed a fabricated signer set — `[{ key: clientAccountID,
   * weight: 1 }]` with `threshold: 1`, where the account ID was read out of the
   * submitted XDR itself. The account was never fetched, so:
   *
   *   - a master key whose weight had been set to 0 (the standard revocation
   *     after a key compromise) still authenticated as that account, and
   *   - any single signer on a multi-sig account satisfied verification,
   *     regardless of the account's actual `med_threshold`.
   *
   * Both matter here because the accounts this gate protects are the treasury
   * and governance accounts, which are exactly the ones expected to be
   * multi-sig.
   */
  static async verifyResponse(
    xdr: string,
    serverAddress: string,
    networkPassphrase: string,
    domain: string,
    loadAccount: AccountLoader,
  ): Promise<string> {
    const { clientAccountID } = WebAuth.readChallengeTx(
      xdr,
      serverAddress,
      networkPassphrase,
      domain,
      domain // webAuthDomain
    );

    let account: AccountSigners;
    try {
      account = await loadAccount(clientAccountID);
    } catch (err) {
      // A missing account cannot be verified against a signer set. Fail closed
      // rather than falling back to the master-key assumption.
      throw new Error(
        `Cannot verify challenge: unable to load account ${clientAccountID}`,
      );
    }

    const signerSummary = account.signers.map((s) => ({
      key: s.key,
      weight: s.weight,
      type: s.type,
    }));

    // med_threshold governs signature-weight operations, which is the category
    // a SEP-10 challenge falls under.
    const threshold = account.thresholds.med_threshold;

    let signersFound: string[];
    try {
      signersFound = WebAuth.verifyChallengeTxThreshold(
        xdr,
        serverAddress,
        networkPassphrase,
        threshold,
        signerSummary,
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
