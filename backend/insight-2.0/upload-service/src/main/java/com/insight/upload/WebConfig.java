package com.insight.upload;

import org.springframework.context.annotation.Configuration;
import org.springframework.web.servlet.config.annotation.CorsRegistry;
import org.springframework.web.servlet.config.annotation.WebMvcConfigurer;

/**
 * Dev-only CORS: allows the Angular dev server (insight-ui, localhost:4200) to
 * call this
 * service's REST API directly from the browser. A real deployment would
 * restrict this to
 * whatever origin actually hosts the UI, likely via configuration rather than a
 * hardcoded value.
 */
@Configuration
public class WebConfig implements WebMvcConfigurer {

        @Override
        public void addCorsMappings(CorsRegistry registry) {
                registry.addMapping("/api/v1/uploads/**")
                                .allowedOrigins(
                                                "http://localhost:4200",
                                                "http://tauri.localhost")
                                .allowedMethods("GET", "POST", "PUT", "OPTIONS")
                                .allowedHeaders("*");
                registry.addMapping("/api/v1/cases/**")
                                .allowedOrigins(
                                                "http://localhost:4200",
                                                "http://tauri.localhost")
                                .allowedMethods("GET", "POST", "OPTIONS")
                                .allowedHeaders("*");
                registry.addMapping("/api/v1/packets/**")
                                .allowedOrigins(
                                                "http://localhost:4200",
                                                "http://tauri.localhost")
                                .allowedMethods("GET", "POST", "OPTIONS")
                                .allowedHeaders("*");
        }
}
