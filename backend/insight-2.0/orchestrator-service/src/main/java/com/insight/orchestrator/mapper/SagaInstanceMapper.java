package com.insight.orchestrator.mapper;

import com.insight.orchestrator.dto.SagaInstanceResponse;
import com.insight.orchestrator.entity.SagaInstance;
import org.springframework.stereotype.Component;

@Component
public class SagaInstanceMapper {

    public SagaInstanceResponse toResponse(SagaInstance saga) {
        return new SagaInstanceResponse(
                saga.getSagaId().toString(),
                saga.getUploadId().toString(),
                saga.getState().name(),
                saga.getCurrentStep(),
                saga.getRetryCount()
        );
    }
}
