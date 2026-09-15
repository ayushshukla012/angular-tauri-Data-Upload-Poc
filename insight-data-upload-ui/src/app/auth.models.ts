export interface BffSession {
  authenticated: boolean;
}

export interface UserContext {
  authenticated: boolean;
  name: string;
  authorities: string[];
  subject: string;
  preferredUsername: string;
  issuer: string;
}

export interface NativeTokenResponse {
  accessToken: string;
  refreshToken?: string | null;
  tokenType?: string;
  expiresIn?: number;
}

export interface StoredTokenSet {
  accessToken: string;
  refreshToken?: string | null;
  expiresAt?: number | null;
  tokenType?: string;
}
