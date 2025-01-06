import { err } from "./fmt";

const D = [
  [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
  [1, 2, 3, 4, 0, 6, 7, 8, 9, 5],
  [2, 3, 4, 0, 1, 7, 8, 9, 5, 6],
  [3, 4, 0, 1, 2, 8, 9, 5, 6, 7],
  [4, 0, 1, 2, 3, 9, 5, 6, 7, 8],
  [5, 9, 8, 7, 6, 0, 4, 3, 2, 1],
  [6, 5, 9, 8, 7, 1, 0, 4, 3, 2],
  [7, 6, 5, 9, 8, 2, 1, 0, 4, 3],
  [8, 7, 6, 5, 9, 3, 2, 1, 0, 4],
  [9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
];

const P = [
  [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
  [1, 5, 7, 6, 2, 8, 3, 0, 9, 4],
  [5, 8, 0, 3, 7, 9, 6, 1, 4, 2],
  [8, 9, 1, 6, 0, 4, 3, 5, 2, 7],
  [9, 4, 5, 3, 1, 2, 6, 8, 7, 0],
  [4, 2, 8, 6, 5, 7, 3, 9, 0, 1],
  [2, 7, 9, 3, 8, 0, 6, 4, 1, 5],
  [7, 0, 4, 6, 9, 1, 3, 2, 5, 8]
];

const INVERSE = [0, 4, 3, 2, 1, 5, 6, 7, 8, 9];

function rawChecksum(digits: string): number {
  return digits.split("").reduceRight((acc, char, i) => {
    let index = P[(digits.length - 1 - i) % 8]![+char];
    if (index === undefined) {
      throw err`Invalid digit ${char}`;
    }
    return D[acc]![index]!;
  }, 0);
}

export function checkDigit(digits: string): number {
  return INVERSE[rawChecksum(`${digits}0`)]!;
}

export function validChecksum(digits: string): boolean {
  return rawChecksum(digits) === 0;
}

const ACCOUNT_ID_LENGTH = 19;
const MAX_ID = 10n ** BigInt(ACCOUNT_ID_LENGTH);
const USER_ACCOUNT_NUMBER_LEN = ACCOUNT_ID_LENGTH + 1;
const ACCOUNT_ID_DISPLAY_CHUNK_SIZE = 4;
const ACCOUNT_ID_CHUNK_RE = new RegExp(`.{${ACCOUNT_ID_DISPLAY_CHUNK_SIZE}}(?=.)`, "g");

function generateAccountId(): BigInt {
  let rand = new BigUint64Array(1);
  while (1) {
    window.crypto.getRandomValues(rand);
    let n = rand[0]!;
    if (n < MAX_ID) {
      return n;
    }
  }
  throw err`unreachable`;
}

export function generateAccountNumber(): AccountId {
  const accountID = generateAccountId().toString().padStart(ACCOUNT_ID_LENGTH, '0');
  return accountID + String(checkDigit(accountID)) as any as AccountId;
}

export interface AccountId {
  readonly Type: unique symbol
};

/// The raw formatting of an account ID.
export function accountIdToString(id: AccountId): string {
	return id as any as string;
}

const enum ObscuraAccountErrorCode {
  TOO_SHORT = "tooShort",
  TOO_LONG = "tooLong",
  INVALID_CHECKSUM = "invalidChecksum",
};

export class ObscuraAccountIdError extends Error {
  public readonly code: string;

  constructor(code: ObscuraAccountErrorCode, message: string) {
    super(message);
    this.name = 'ObscuraAccountError';
    this.code = code;
  }

  i18nKey() {
    return `accountIdError-${this.code}`;
  }
}

/// Parse a strictly integer account ID.
export function parseAccountIdInt(id: string): AccountId {
  if (id.length < USER_ACCOUNT_NUMBER_LEN) {
    throw new ObscuraAccountIdError(ObscuraAccountErrorCode.TOO_SHORT, "Account ID is too short.");
  }
  if (id.length > USER_ACCOUNT_NUMBER_LEN) {
    throw new ObscuraAccountIdError(ObscuraAccountErrorCode.TOO_LONG, "Account ID is too long.");
  }
  if (!validChecksum(id)) {
    throw new ObscuraAccountIdError(ObscuraAccountErrorCode.INVALID_CHECKSUM, "Mistyped Account ID.");
  }
  return id as any as AccountId;
}

export function parseAccountIdInput(input: string): AccountId {
  return parseAccountIdInt(normalizeAccountIdInput(input));
}

function normalizeAccountIdInput(id: string): string {
  return id.replace(/[^\d]/g, "");
}

export function formatPartialAccountId(accountId: string): string {
  accountId = normalizeAccountIdInput(accountId);
  if (accountId.length >= USER_ACCOUNT_NUMBER_LEN) {
    return `${accountId.slice(0, 4)} - ${accountId.slice(4, 8)} - ${accountId.slice(8, 12)} - ${accountId.slice(12, 16)} - ${accountId.slice(16)}`;
  }
  return accountId.replace(ACCOUNT_ID_CHUNK_RE, "$& - ");
}

export const OBSCURA_WEBPAGE = 'https://obscura.net';
export const CHECK_STATUS_WEBPAGE = `${OBSCURA_WEBPAGE}/check`;
export const LEGAL_WEBPAGE = `${OBSCURA_WEBPAGE}/legal`;
export const TERMS_WEBPAGE = `${OBSCURA_WEBPAGE}/legal#terms-of-service`;

export function payUrl(accountId: AccountId): string {
  return `${OBSCURA_WEBPAGE}/pay#account_id=${encodeURIComponent(String(accountId))}`;
}

export function subscriptionUrl(accountId: AccountId): string {
  return `${OBSCURA_WEBPAGE}/subscription/stripe/checkout#account_id=${encodeURIComponent(String(accountId))}`;
}
