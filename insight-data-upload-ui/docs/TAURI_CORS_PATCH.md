# Tauri CORS patch for the supplied Spring Boot backend

The supplied backend currently allows only `http://localhost:4200` in `WebConfig.java` for the `/api/v1/uploads/**`, `/api/v1/cases/**`, and `/api/v1/packets/**` routes.

Tauri 2 production webviews use `http://tauri.localhost` on Windows with the default HTTP custom protocol scheme.

For local browser + desktop development, allow both origins explicitly.

## `upload-service/src/main/java/com/insight/upload/WebConfig.java`

Replace each:

```java
.allowedOrigins("http://localhost:4200")
```

with:

```java
.allowedOrigins(
    "http://localhost:4200",
    "http://tauri.localhost"
)
```

Keep the allowed methods and headers unchanged.

## `application.yml` actuator CORS

The supplied `application.yml` currently contains:

```yaml
management:
  endpoints:
    web:
      cors:
        allowed-origins: [http://localhost:4200]
        allowed-methods: GET
```

Change it to:

```yaml
management:
  endpoints:
    web:
      cors:
        allowed-origins: [http://localhost:4200, http://tauri.localhost]
        allowed-methods: GET
```

## `docker-compose.yml` MinIO CORS

The supplied local MinIO service uses:

```yaml
environment:
  MINIO_API_CORS_ALLOW_ORIGIN: "[http://localhost:4200]"
```

For browser + packaged Tauri development, allow both origins:

```yaml
environment:
  MINIO_API_CORS_ALLOW_ORIGIN: "[http://localhost:4200,http://tauri.localhost]"
```

Do not replace this with `*` for a government/security-sensitive utility.

After changing the MinIO configuration, recreate the MinIO container so the environment is applied.
