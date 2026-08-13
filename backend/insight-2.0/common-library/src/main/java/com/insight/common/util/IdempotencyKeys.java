package com.insight.common.util;

public final class IdempotencyKeys {

    private IdempotencyKeys() {
    }

    public static String forSagaStep(String sagaId, String stepName) {
        return "%s:%s".formatted(sagaId, stepName);
    }
}
