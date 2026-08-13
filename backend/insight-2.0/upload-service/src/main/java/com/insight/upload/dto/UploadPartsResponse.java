package com.insight.upload.dto;

import java.util.List;

public record UploadPartsResponse(
        List<UploadedPartSummary> parts
) {}
