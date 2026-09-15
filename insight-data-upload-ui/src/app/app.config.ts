import { ApplicationConfig, inject, provideAppInitializer } from '@angular/core';
import { provideHttpClient, withXsrfConfiguration } from '@angular/common/http';
import { provideAnimationsAsync } from '@angular/platform-browser/animations/async';

import { AppConfigService } from './core/config/app-config.service';

export const appConfig: ApplicationConfig = {
  providers: [
    provideHttpClient(
      withXsrfConfiguration({
        cookieName: 'XSRF-TOKEN',
        headerName: 'X-XSRF-TOKEN'
      })
    ),
    provideAnimationsAsync(),
    provideAppInitializer(() =>
      inject(AppConfigService).load()
    )
  ]
};
