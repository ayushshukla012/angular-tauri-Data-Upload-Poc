package com.insight.transformation.service;

public final class KafkaTopics {

    public static final String INTERNAL_START = "transformation.internal.start";
    public static final String COMPLETED = "transformation.events.completed";
    public static final String FAILED = "transformation.events.failed";

    private KafkaTopics() {
    }
}
