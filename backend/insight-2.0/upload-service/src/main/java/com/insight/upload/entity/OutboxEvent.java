package com.insight.upload.entity;

import jakarta.persistence.*;
import java.time.Instant;
import java.util.UUID;

@Entity
@Table(name = "outbox_events")
public class OutboxEvent {

    @Id
    private UUID id;

    @Column(nullable = false)
    private String topic;

    @Column(nullable = false)
    private String aggregateId;

    @Column(nullable = false, columnDefinition = "TEXT")
    private String payload;

    @Column(nullable = false)
    private boolean published;

    @Column(nullable = false)
    private Instant createdAt;

    protected OutboxEvent() {
    }

    public OutboxEvent(UUID id, String topic, String aggregateId, String payload) {
        this.id = id;
        this.topic = topic;
        this.aggregateId = aggregateId;
        this.payload = payload;
        this.published = false;
        this.createdAt = Instant.now();
    }

    public UUID getId() { return id; }
    public String getTopic() { return topic; }
    public String getAggregateId() { return aggregateId; }
    public String getPayload() { return payload; }
    public boolean isPublished() { return published; }
    public void markPublished() { this.published = true; }
}
