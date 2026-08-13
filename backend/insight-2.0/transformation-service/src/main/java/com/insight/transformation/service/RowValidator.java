package com.insight.transformation.service;

import org.springframework.stereotype.Component;

import java.util.Map;
import java.util.Optional;

/**
 * Placeholder rule set: rejects rows with any blank field. Real per-file-type schemas
 * (required columns, type coercion, business rules) plug in here without touching the
 * worker or the file processors.
 */
@Component
public class RowValidator {

    public Optional<String> validate(Map<String, String> fields) {
        for (Map.Entry<String, String> field : fields.entrySet()) {
            if (field.getValue() == null || field.getValue().isBlank()) {
                return Optional.of("Field '%s' must not be blank".formatted(field.getKey()));
            }
        }
        return Optional.empty();
    }
}
