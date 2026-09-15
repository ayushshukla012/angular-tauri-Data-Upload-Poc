import { Injectable, inject, signal } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { firstValueFrom } from 'rxjs';
import { environment } from '../../environments/environment';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { BffSession, NativeTokenResponse, StoredTokenSet, UserContext } from '../auth.models';

interface OAuthCallbackEvent {
  url: string;
}

@Injectable({ providedIn: 'root' })
export class AuthService {
  private readonly http = inject(HttpClient);
  private readonly bffUrl = (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window)
    ? environment.bffBaseUrl.replace(/\/$/, '')
    : '/bff';

  readonly authenticated = signal(false);
  readonly user = signal<UserContext | null>(null);
  readonly busy = signal(false);
  readonly error = signal<string | null>(null);

  private accessToken: string | null = null;
  private refreshToken: string | null = null;
  private expiresAt = 0;

  isNative(): boolean {
    return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
  }

  async bootstrap(): Promise<void> {
    this.error.set(null);
    if (this.isNative()) {
      const stored = await this.readStoredTokens();
      if (stored?.accessToken) {
        this.setTokens(stored);
        if (!this.isAccessTokenExpired()) {
          await this.loadUserFromBff();
          return;
        }
        if (stored.refreshToken) {
          try {
            await this.refreshNativeTokens(stored.refreshToken);
            await this.loadUserFromBff();
            return;
          } catch {
            await this.clearNativeTokens();
            this.accessToken = null;
            this.refreshToken = null;
            this.expiresAt = 0;
          }
        }
      }
      this.authenticated.set(false);
      return;
    }

    try {
      await this.initializeCsrf();
      const session = await this.checkSession();
      this.authenticated.set(session.authenticated);
      if (session.authenticated) await this.loadUser();
    } catch {
      this.authenticated.set(false);
    }
  }

  checkSession() {
    return firstValueFrom(this.http.get<BffSession>(`${this.bffUrl}/api/bff/session`, { withCredentials: true }));
  }

  initializeCsrf() {
    return firstValueFrom(this.http.get<{ headerName: string }>(`${this.bffUrl}/api/bff/csrf`, { withCredentials: true }));
  }

  loadUser() {
    return firstValueFrom(this.http.get<UserContext>(`${this.bffUrl}/api/bff/me`, { withCredentials: true })).then(user => {
      this.user.set(user);
      this.authenticated.set(!!user.authenticated);
      return user;
    });
  }

  hasAuthority(authority: string): boolean {
    return this.user()?.authorities?.includes(authority) ?? false;
  }

  async ensureAuthenticated(): Promise<boolean> {
    if (this.authenticated() && !this.isNative()) return true;
    if (this.isNative()) {
      if (this.accessToken && !this.isAccessTokenExpired()) return true;
      if (this.refreshToken) {
        try {
          await this.refreshNativeTokens(this.refreshToken);
          await this.loadUserFromBff();
          return this.authenticated();
        } catch {
          await this.clearNativeTokens();
        }
      }
    }

    this.busy.set(true);
    this.error.set(null);
    try {
      if (this.isNative()) {
        await this.nativeLogin();
      } else {
        this.login();
        return false;
      }
      return this.authenticated();
    } catch (error) {
      this.error.set(error instanceof Error ? error.message : 'Authentication failed.');
      throw error;
    } finally {
      this.busy.set(false);
    }
  }

  login(): void {
    // Browser deployment uses the BFF's normal session-based OAuth flow.
    window.location.href = `${this.bffUrl}/oauth2/authorization/insight-bff`;
  }

  async logout(): Promise<void> {
    this.error.set(null);
    try {
      if (this.isNative()) {
        await this.clearNativeTokens();
        this.accessToken = null;
        this.refreshToken = null;
        this.expiresAt = 0;
        this.authenticated.set(false);
        this.user.set(null);
        return;
      }
      await firstValueFrom(this.http.post<void>(`${this.bffUrl}/api/bff/logout`, {}, { withCredentials: true }));
      this.authenticated.set(false);
      this.user.set(null);
    } catch (error) {
      this.error.set(error instanceof Error ? error.message : 'Logout failed.');
      throw error;
    }
  }

  /** Access token is held in memory only; the persistent copy stays in the OS credential store. */
  getAccessToken(): string | null {
    return this.isNative() ? this.accessToken : null;
  }

  async ensureValidAccessToken(): Promise<boolean> {
    if (!this.isNative()) return this.authenticated();
    if (this.accessToken && !this.isAccessTokenExpired()) return true;
    if (!this.refreshToken) return false;
    try {
      await this.refreshNativeTokens(this.refreshToken);
      return true;
    } catch {
      await this.clearNativeTokens();
      this.accessToken = null;
      this.refreshToken = null;
      this.expiresAt = 0;
      this.authenticated.set(false);
      this.user.set(null);
      return false;
    }
  }

  private async nativeLogin(): Promise<void> {
    const state = this.randomString(32);
    const verifier = this.randomString(64);
    const challenge = await this.sha256Base64Url(verifier);

    // RFC 8252: native apps should use the system browser and a loopback redirect.
    const redirectUri = await invoke<string>('prepare_oauth_callback', { path: '/oauth/callback' });

    // IMPORTANT: start listening for the callback without awaiting the callback itself.
    // waitForOAuthCallback() only resolves after the browser returns. Awaiting it here
    // would block before open_external_url() is called, leaving the UI stuck on "Signing in…".
    const callbackPromise = this.waitForOAuthCallback(state);

    // The BFF must preserve redirect_uri/state/PKCE through to Keycloak for this native flow.
    const loginUrl = new URL(`${this.bffUrl}/oauth2/authorization/insight-bff`);
    loginUrl.searchParams.set('redirect_uri', redirectUri);
    loginUrl.searchParams.set('state', state);
    loginUrl.searchParams.set('code_challenge', challenge);
    loginUrl.searchParams.set('code_challenge_method', 'S256');
    loginUrl.searchParams.set('native', 'true');

    await invoke('open_external_url', { url: loginUrl.toString() });

    const callbackUrl = await callbackPromise;
    const callback = new URL(callbackUrl);
    if (callback.searchParams.get('state') !== state) throw new Error('OAuth state validation failed.');
    const error = callback.searchParams.get('error');
    if (error) throw new Error(callback.searchParams.get('error_description') || `Login failed: ${error}`);

    const code = callback.searchParams.get('code');
    if (!code) throw new Error('Keycloak did not return an authorization code.');

    // REQUIRED BFF endpoint: exchange the authorization code at Keycloak and return tokens.
    // This keeps the Keycloak client integration behind the BFF instead of exposing credentials in Angular.
    const tokens = await firstValueFrom(this.http.post<NativeTokenResponse>(`${this.bffUrl}/api/bff/native/token`, {
      code,
      redirectUri,
      codeVerifier: verifier,
      state
    }, { withCredentials: true }));

    await this.storeTokens({
      accessToken: tokens.accessToken,
      refreshToken: tokens.refreshToken,
      expiresAt: Date.now() + Math.max(30, tokens.expiresIn ?? 300) * 1000,
      tokenType: tokens.tokenType || 'Bearer'
    });
    await this.loadUserFromBff();
  }

  private async refreshNativeTokens(refreshToken: string): Promise<void> {
    // REQUIRED BFF endpoint for native refresh. Do not place refresh tokens in Web Storage.
    const tokens = await firstValueFrom(this.http.post<NativeTokenResponse>(`${this.bffUrl}/api/bff/native/refresh`, {
      refreshToken
    }));
    await this.storeTokens({
      accessToken: tokens.accessToken,
      refreshToken: tokens.refreshToken || refreshToken,
      expiresAt: Date.now() + Math.max(30, tokens.expiresIn ?? 300) * 1000,
      tokenType: tokens.tokenType || 'Bearer'
    });
  }

  private async loadUserFromBff(): Promise<void> {
    const user = await firstValueFrom(this.http.get<UserContext>(`${this.bffUrl}/api/bff/me`, {
      headers: this.accessToken ? { Authorization: `Bearer ${this.accessToken}` } : undefined,
      withCredentials: true
    }));
    this.user.set(user);
    this.authenticated.set(!!user.authenticated);
  }

  private async waitForOAuthCallback(expectedState: string): Promise<Promise<string>> {
    let resolveCallback!: (value: string | PromiseLike<string>) => void;
    let rejectCallback!: (reason?: unknown) => void;
    let finished = false;
    let unlisten: (() => void) | undefined;

    const promise = new Promise<string>((resolve, reject) => {
      resolveCallback = resolve;
      rejectCallback = reject;
    });

    unlisten = await listen<OAuthCallbackEvent>('oauth-callback', event => {
      try {
        const callbackUrl = new URL(event.payload.url);
        if (callbackUrl.searchParams.get('state') !== expectedState) return;
        if (finished) return;
        finished = true;
        void unlisten?.();
        resolveCallback(event.payload.url);
      } catch (error) {
        if (finished) return;
        finished = true;
        void unlisten?.();
        rejectCallback(error);
      }
    });

    const timeout = window.setTimeout(() => {
      if (finished) return;
      finished = true;
      void unlisten?.();
      rejectCallback(new Error('Timed out waiting for the browser login callback.'));
    }, 5 * 60 * 1000);

    // Clear the timer once the callback settles.
    void promise.then(() => window.clearTimeout(timeout), () => window.clearTimeout(timeout));
    return promise;
  }

  private async readStoredTokens(): Promise<StoredTokenSet | null> {
    if (!this.isNative()) return null;
    try { return await invoke<StoredTokenSet | null>('get_oauth_tokens'); } catch { return null; }
  }

  private async storeTokens(tokens: StoredTokenSet): Promise<void> {
    this.setTokens(tokens);
    if (!this.isNative()) return;
    await invoke('store_oauth_tokens', {
      accessToken: tokens.accessToken,
      refreshToken: tokens.refreshToken ?? null,
      expiresAt: tokens.expiresAt ?? null,
      tokenType: tokens.tokenType ?? 'Bearer'
    });
  }

  private async clearNativeTokens(): Promise<void> {
    if (!this.isNative()) return;
    try { await invoke('clear_oauth_tokens'); } catch { /* best effort */ }
  }

  private setTokens(tokens: StoredTokenSet): void {
    this.accessToken = tokens.accessToken;
    this.refreshToken = tokens.refreshToken ?? null;
    this.expiresAt = tokens.expiresAt ?? 0;
  }

  private isAccessTokenExpired(): boolean {
    return !this.accessToken || (this.expiresAt > 0 && Date.now() >= this.expiresAt - 60_000);
  }

  private randomString(length: number): string {
    const bytes = new Uint8Array(length);
    crypto.getRandomValues(bytes);
    return Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
  }

  private async sha256Base64Url(value: string): Promise<string> {
    const data = new TextEncoder().encode(value);
    const digest = await crypto.subtle.digest('SHA-256', data);
    let binary = '';
    for (const byte of new Uint8Array(digest)) binary += String.fromCharCode(byte);
    return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '');
  }
}
