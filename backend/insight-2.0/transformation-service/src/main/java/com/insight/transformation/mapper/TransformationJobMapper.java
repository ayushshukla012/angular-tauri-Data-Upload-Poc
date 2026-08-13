package com.insight.transformation.mapper;

import com.insight.transformation.dto.TransformationJobResponse;
import com.insight.transformation.entity.TransformationJob;
import org.springframework.stereotype.Component;

@Component
public class TransformationJobMapper {

    public TransformationJobResponse toResponse(TransformationJob job) {
        return new TransformationJobResponse(
                job.getUploadId().toString(),
                job.getStatus().name(),
                job.getTotalRows(),
                job.getValidRows(),
                job.getErrorRows(),
                job.isRequiresOcr()
        );
    }
}
