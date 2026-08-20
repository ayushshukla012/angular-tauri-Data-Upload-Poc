import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { firstValueFrom } from 'rxjs';
import { AppConfig } from './app-config.models';

@Injectable({ providedIn: 'root' })
export class AppConfigService {
  private readonly http = inject(HttpClient);
  private configValue: AppConfig | null = null;

  async load(): Promise<void> {
    this.configValue = await firstValueFrom(
      this.http.get<AppConfig>('/assets/config/app-config.json')
    );

    if (!this.configValue?.pagination?.defaultPageSize) {
      throw new Error('Invalid runtime configuration: pagination.defaultPageSize is required.');
    }
  }

  get value(): AppConfig {
    if (!this.configValue) {
      throw new Error('Runtime configuration has not loaded.');
    }
    return this.configValue;
  }
}

export function initializeAppConfig(
  config: AppConfigService
): () => Promise<void> {
  return () => config.load();
}
