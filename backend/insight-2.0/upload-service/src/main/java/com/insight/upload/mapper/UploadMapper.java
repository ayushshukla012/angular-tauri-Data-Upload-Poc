package com.insight.upload.mapper;

import com.insight.upload.dto.UploadResponse;
import com.insight.upload.entity.Upload;
import org.springframework.stereotype.Component;

@Component
public class UploadMapper {

    public UploadResponse toResponse(Upload upload) {
        return new UploadResponse(
                upload.getId().toString(),
                upload.getFileName(),
                upload.getFileType(),
                upload.getStatus().name(),
                upload.getCreatedAt()
        );
    }
}
