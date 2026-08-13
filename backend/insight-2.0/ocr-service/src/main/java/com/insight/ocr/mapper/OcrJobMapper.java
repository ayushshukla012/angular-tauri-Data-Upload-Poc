package com.insight.ocr.mapper;

import com.insight.ocr.dto.OcrJobResponse;
import com.insight.ocr.entity.OcrJob;
import org.springframework.stereotype.Component;

@Component
public class OcrJobMapper {

    public OcrJobResponse toResponse(OcrJob job) {
        return new OcrJobResponse(
                job.getUploadId().toString(),
                job.getStatus().name(),
                job.getDocumentCount(),
                job.getExtractedCount()
        );
    }
}
